use crate::types::{FieldId, FieldStats, NUM_FIELDS};

const K1: f32 = 1.2;
const B: f32 = 0.75;

/// Default field weights.
/// Index: FIELD_SYMBOL_NAME=0, FIELD_DOC_CONTENT=1, FIELD_PARAMS=2, FIELD_FILEPATH=3
const DEFAULT_WEIGHTS: [f32; NUM_FIELDS] = [4.0, 2.0, 1.5, 1.0];

pub struct Bm25Scorer {
    weights: [f32; NUM_FIELDS],
}

impl Bm25Scorer {
    pub fn new() -> Self {
        Self {
            weights: DEFAULT_WEIGHTS,
        }
    }

    pub fn with_weights(weights: [f32; NUM_FIELDS]) -> Self {
        Self { weights }
    }

    pub fn weight(&self, field_id: FieldId) -> f32 {
        self.weights[field_id as usize]
    }

    /// Compute IDF for a term given its document frequency and total doc count.
    pub fn idf(&self, doc_freq: u32, total_docs: u32) -> f32 {
        let n = doc_freq as f64;
        let total = total_docs as f64;
        ((total - n + 0.5) / (n + 0.5) + 1.0).ln() as f32
    }

    /// Compute TF component with length normalization for a specific field.
    pub fn tf(&self, term_freq: u16, field_length: u16, field_stats: &FieldStats) -> f32 {
        let tf = term_freq as f32;
        let dl = field_length as f32;
        let avgdl = field_stats.avg_length();
        if avgdl == 0.0 {
            return 0.0;
        }
        (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avgdl))
    }

    /// Compute the full BM25 score for a single term hit in a specific field.
    pub fn score(
        &self,
        term_freq: u16,
        field_length: u16,
        field_id: FieldId,
        field_stats: &FieldStats,
        doc_freq: u32,
        total_docs: u32,
    ) -> f32 {
        let idf = self.idf(doc_freq, total_docs);
        let tf = self.tf(term_freq, field_length, field_stats);
        let weight = self.weight(field_id);
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
        let score_name = scorer.score(1, 10, 0, &stats, 5, 100); // FIELD_SYMBOL_NAME, weight 4.0
        let score_path = scorer.score(1, 10, 3, &stats, 5, 100); // FIELD_FILEPATH, weight 1.0
        assert!((score_name / score_path - 4.0).abs() < 0.01);
    }
}
