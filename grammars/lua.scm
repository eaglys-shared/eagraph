; Function declarations
(function_declaration
  name: (identifier) @func.name) @func.def

; Function declarations with dot syntax
(function_declaration
  name: (dot_index_expression
    field: (identifier) @func.name)) @func.def

; Function declarations with colon syntax (methods)
(function_declaration
  name: (method_index_expression
    method: (identifier) @func.name)) @func.def

; Function calls
(function_call
  name: (identifier) @call.name) @call.def

; Method calls
(function_call
  name: (method_index_expression
    table: (_) @method_call.object
    method: (identifier) @method_call.name)) @method_call.def

; Require calls
(function_call
  name: (identifier) @_req
  arguments: (arguments (string) @import.module)
  (#eq? @_req "require")) @import.def
