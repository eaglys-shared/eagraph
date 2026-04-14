; Function declarations
(function_declaration
  (simple_identifier) @func.name) @func.def

; Class declarations
(class_declaration
  (type_identifier) @class.name) @class.def

; Object declarations
(object_declaration
  (type_identifier) @class.name) @class.def

; Interface (via class with modifier)
(class_declaration
  (type_identifier) @class.name) @class.def

; Delegation specifiers (inheritance)
(delegation_specifier
  (user_type
    (type_identifier) @class.bases))

; Function calls
(call_expression
  (simple_identifier) @call.name) @call.def

; Method calls
(call_expression
  (navigation_expression
    (_) @method_call.object
    (simple_identifier) @method_call.name)) @method_call.def

; Import statements
(import_header
  (identifier) @import.module) @import.def
