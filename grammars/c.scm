; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @func.name)) @func.def

; Function declarations
(declaration
  declarator: (function_declarator
    declarator: (identifier) @func.name)) @func.def

; Struct definitions
(struct_specifier
  name: (type_identifier) @class.name) @class.def

; Enum definitions
(enum_specifier
  name: (type_identifier) @class.name) @class.def

; Typedef
(type_definition
  declarator: (type_identifier) @class.name) @class.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Preproc includes
(preproc_include
  path: (_) @import.module) @import.def
