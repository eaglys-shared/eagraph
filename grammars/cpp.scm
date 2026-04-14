; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @func.name)) @func.def

; Method definitions (qualified)
(function_definition
  declarator: (function_declarator
    declarator: (qualified_identifier
      name: (identifier) @func.name))) @func.def

; Class definitions
(class_specifier
  name: (type_identifier) @class.name) @class.def

; Struct definitions
(struct_specifier
  name: (type_identifier) @class.name) @class.def

; Base class specifiers
(base_class_clause
  (type_identifier) @class.bases)

; Enum definitions
(enum_specifier
  name: (type_identifier) @class.name) @class.def

; Namespace definitions
(namespace_definition
  name: (identifier) @class.name) @class.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Method calls
(call_expression
  function: (field_expression
    argument: (_) @method_call.object
    field: (field_identifier) @method_call.name)) @method_call.def

; Includes
(preproc_include
  path: (_) @import.module) @import.def

; Using declarations
(using_declaration
  (_) @import.module) @import.def
