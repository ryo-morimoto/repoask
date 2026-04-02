use crate::index::{FieldId, FieldStats, NUM_FIELDS};

const K1: f32 = 1.2;
const B: f32 = 0.75;

/// Default field weights.
/// Index: `FIELD_SYMBOL_NAME=0`, `FIELD_DOC_CONTENT=1`, `FIELD_PARAMS=2`, `FIELD_FILEPATH=3`
const DEFAULT_WEIGHTS: [f32; NUM_FIELDS] = [4.0, 2.0, 1.5, 1.0];

/// BM25 scorer with per-field weight support.
pub struct Bm25Scorer {
    weights: [f32; NUM_FIELDS],
}

/// Inputs for scoring a single term hit in one document field.
#[derive(Clone, Copy)]
pub struct ScoreInput<'a> {
    /// How many times the term appears in the field.
    pub term_freq: u16,
    /// Number of tokens in the field.
    pub field_length: u16,
    /// Which logical field the hit belongs to.
    pub field_id: FieldId,
    /// Aggregate corpus stats for that field.
    pub field_stats: &'a FieldStats,
    /// Number of documents containing the term.
    pub doc_freq: u32,
    /// Total number of indexed documents.
    pub total_docs: u32,
}

impl Bm25Scorer {
    /// Create a scorer with default field weights.
    pub const fn new() -> Self {
        Self {
            weights: DEFAULT_WEIGHTS,
        }
    }

    /// Create a scorer with custom field weights.
    pub const fn with_weights(weights: [f32; NUM_FIELDS]) -> Self {
        Self { weights }
    }

    /// Return the weight multiplier for the given field.
    pub fn weight(&self, field_id: FieldId) -> f32 {
        self.weights
            .get(usize::from(field_id))
            .copied()
            .unwrap_or(0.0)
    }

    /// Compute IDF for a term given its document frequency and total doc count.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "BM25 scores are intentionally stored as f32 throughout the index"
    )]
    pub fn idf(&self, doc_freq: u32, total_docs: u32) -> f32 {
        let n = f64::from(doc_freq);
        let total = f64::from(total_docs);
        ((total - n + 0.5) / (n + 0.5)).ln_1p() as f32
    }

    /// Compute TF component with length normalization for a specific field.
    pub fn tf(&self, term_freq: u16, field_length: u16, field_stats: &FieldStats) -> f32 {
        let tf = f32::from(term_freq);
        let dl = f32::from(field_length);
        let avgdl = field_stats.avg_length();
        if avgdl == 0.0 {
            return 0.0;
        }
        (tf * (K1 + 1.0)) / K1.mul_add(1.0 - B + B * dl / avgdl, tf)
    }

    /// Compute the full BM25 score for a single term hit in a specific field.
    pub fn score(&self, input: ScoreInput<'_>) -> f32 {
        let idf = self.idf(input.doc_freq, input.total_docs);
        let tf = self.tf(input.term_freq, input.field_length, input.field_stats);
        let weight = self.weight(input.field_id);
        idf * tf * weight
    }
}

impl Default for Bm25Scorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idf_common_term() {
        let scorer = Bm25Scorer::new();
        // Term appearing in half the docs should have low IDF
        let idf = scorer.idf(50, 100);
        assert!(idf > 0.0);
        assert!(idf < 1.0);
    }

    #[test]
    fn test_idf_rare_term() {
        let scorer = Bm25Scorer::new();
        // Term appearing in 1 of 1000 docs should have high IDF
        let idf = scorer.idf(1, 1000);
        assert!(idf > 5.0);
    }

    #[test]
    fn test_tf_saturation() {
        let scorer = Bm25Scorer::new();
        let stats = FieldStats {
            total_length: 100,
            doc_count: 10,
        };
        let tf1 = scorer.tf(1, 10, &stats);
        let tf10 = scorer.tf(10, 10, &stats);
        // TF should saturate: 10x frequency should NOT yield 10x score
        assert!(tf10 / tf1 < 5.0);
    }

    #[test]
    fn test_field_weight_matters() {
        let scorer = Bm25Scorer::new();
        let stats = FieldStats {
            total_length: 100,
            doc_count: 10,
        };
        let score_name = scorer.score(ScoreInput {
            term_freq: 1,
            field_length: 10,
            field_id: 0,
            field_stats: &stats,
            doc_freq: 5,
            total_docs: 100,
        }); // FIELD_SYMBOL_NAME, weight 4.0
        let score_path = scorer.score(ScoreInput {
            term_freq: 1,
            field_length: 10,
            field_id: 3,
            field_stats: &stats,
            doc_freq: 5,
            total_docs: 100,
        }); // FIELD_FILEPATH, weight 1.0
        assert!((score_name / score_path - 4.0).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // Property-based tests (proptest)
    // -----------------------------------------------------------------------

    mod property {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// IDF is always non-negative.
            #[test]
            fn idf_non_negative(
                doc_freq in 1u32..1000,
                total_docs in 1u32..10000,
            ) {
                prop_assume!(doc_freq <= total_docs);
                let scorer = Bm25Scorer::new();
                let idf = scorer.idf(doc_freq, total_docs);
                prop_assert!(idf >= 0.0, "negative IDF: {idf} (df={doc_freq}, N={total_docs})");
            }

            /// TF is always non-negative.
            #[test]
            fn tf_non_negative(
                term_freq in 1u16..100,
                field_length in 1u16..1000,
            ) {
                let scorer = Bm25Scorer::new();
                let stats = FieldStats {
                    total_length: 500,
                    doc_count: 10,
                };
                let tf = scorer.tf(term_freq, field_length, &stats);
                prop_assert!(tf >= 0.0, "negative TF: {tf}");
            }

            /// Higher term frequency → higher or equal TF score (monotonic).
            #[test]
            fn tf_monotonic_in_freq(
                tf_low in 1u16..50,
                tf_delta in 1u16..50,
                field_length in 1u16..500,
            ) {
                let tf_high = tf_low.saturating_add(tf_delta);
                let scorer = Bm25Scorer::new();
                let stats = FieldStats {
                    total_length: 500,
                    doc_count: 10,
                };
                let score_low = scorer.tf(tf_low, field_length, &stats);
                let score_high = scorer.tf(tf_high, field_length, &stats);
                prop_assert!(
                    score_high >= score_low,
                    "TF not monotonic: tf({tf_low})={score_low} > tf({tf_high})={score_high}"
                );
            }

            /// Rarer terms (lower doc_freq) have higher IDF.
            #[test]
            fn rarer_terms_higher_idf(
                df_rare in 1u32..100,
                df_delta in 1u32..100,
                total_docs in 200u32..10000,
            ) {
                let df_common = df_rare.saturating_add(df_delta).min(total_docs);
                prop_assume!(df_rare < df_common);
                let scorer = Bm25Scorer::new();
                let idf_rare = scorer.idf(df_rare, total_docs);
                let idf_common = scorer.idf(df_common, total_docs);
                prop_assert!(
                    idf_rare >= idf_common,
                    "rare term should have higher IDF: idf({df_rare})={idf_rare} < idf({df_common})={idf_common}"
                );
            }
        }
    }
}
