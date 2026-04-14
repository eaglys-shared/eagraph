; Function declarations
(function_declaration
  name: (identifier) @func.name) @func.def

; Arrow functions assigned to variables
(lexical_declaration
  (variable_declarator
    name: (identifier) @func.name
    value: (arrow_function))) @func.def

; Method definitions
(method_definition
  name: (property_identifier) @func.name) @func.def

; Class declarations
(class_declaration
  name: (type_identifier) @class.name) @class.def

; Interface declarations
(interface_declaration
  name: (type_identifier) @class.name) @class.def

; Type alias declarations
(type_alias_declaration
  name: (type_identifier) @class.name) @class.def

; Class heritage (extends)
(extends_clause
  value: (identifier) @class.bases)

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Method calls
(call_expression
  function: (member_expression
    object: (_) @method_call.object
    property: (property_identifier) @method_call.name)) @method_call.def

; Import statements
(import_statement
  source: (string) @import.module) @import.def
