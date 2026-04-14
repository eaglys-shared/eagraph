; Function declarations
(function_declaration
  name: (identifier) @func.name) @func.def

; Arrow functions assigned to variables
(lexical_declaration
  (variable_declarator
    name: (identifier) @func.name
    value: (arrow_function))) @func.def

; Method definitions in classes
(method_definition
  name: (property_identifier) @func.name) @func.def

; Class declarations
(class_declaration
  name: (identifier) @class.name) @class.def

; Class heritage
(class_heritage
  (identifier) @class.bases)

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Method calls
(call_expression
  function: (member_expression
    property: (property_identifier) @method_call.name)) @method_call.def

; Import statements: import x from 'y'
(import_statement
  source: (string) @import.module) @import.def

; Require calls: const x = require('y')
(call_expression
  function: (identifier) @_req
  arguments: (arguments (string) @import.module)
  (#eq? @_req "require")) @import.def
