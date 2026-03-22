//! Tree-sitter query strings for each supported language.

pub const RUST: &str = r#"
(function_item
  name: (identifier) @name
  parameters: (parameters) @params) @definition.function

(struct_item
  name: (type_identifier) @name) @definition.struct

(enum_item
  name: (type_identifier) @name) @definition.enum

(trait_item
  name: (type_identifier) @name) @definition.trait

(type_item
  name: (type_identifier) @name) @definition.type

(const_item
  name: (identifier) @name) @definition.const

(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name
      parameters: (parameters) @params) @definition.method))
"#;

pub const PYTHON: &str = r#"
(function_definition
  name: (identifier) @name
  parameters: (parameters) @params) @definition.function

(class_definition
  name: (identifier) @name) @definition.class
"#;

pub const GO: &str = r#"
(function_declaration
  name: (identifier) @name
  parameters: (parameter_list) @params) @definition.function

(method_declaration
  name: (field_identifier) @name
  parameters: (parameter_list) @params) @definition.method

(type_declaration
  (type_spec
    name: (type_identifier) @name) @definition.type)
"#;

pub const JAVA: &str = r#"
(method_declaration
  name: (identifier) @name
  parameters: (formal_parameters) @params) @definition.method

(class_declaration
  name: (identifier) @name) @definition.class

(interface_declaration
  name: (identifier) @name) @definition.interface

(enum_declaration
  name: (identifier) @name) @definition.enum
"#;

pub const C: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name
    parameters: (parameter_list) @params)) @definition.function

(struct_specifier
  name: (type_identifier) @name) @definition.struct

(enum_specifier
  name: (type_identifier) @name) @definition.enum
"#;

pub const CPP: &str = r#"
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @name
    parameters: (parameter_list) @params)) @definition.function

(class_specifier
  name: (type_identifier) @name) @definition.class

(struct_specifier
  name: (type_identifier) @name) @definition.struct

(enum_specifier
  name: (type_identifier) @name) @definition.enum
"#;

pub const RUBY: &str = r#"
(method
  name: (identifier) @name
  parameters: (method_parameters) @params) @definition.method

(class
  name: (constant) @name) @definition.class

(module
  name: (constant) @name) @definition.class
"#;
