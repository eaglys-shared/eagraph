; Function declarations
(function_declaration
  name: (simple_identifier) @func.name) @func.def

; Initializer declarations
(init_declaration) @func.def

; Class declarations
(class_declaration
  name: (type_identifier) @class.name) @class.def

; Struct declarations
(struct_declaration
  name: (type_identifier) @class.name) @class.def

; Protocol declarations
(protocol_declaration
  name: (type_identifier) @class.name) @class.def

; Enum declarations
(enum_declaration
  name: (type_identifier) @class.name) @class.def

; Inheritance
(inheritance_specifier
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

; Import declarations
(import_declaration
  (identifier) @import.module) @import.def
