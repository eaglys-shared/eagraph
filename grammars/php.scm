; Function definitions
(function_definition
  name: (name) @func.name) @func.def

; Method declarations
(method_declaration
  name: (name) @func.name) @func.def

; Class declarations
(class_declaration
  name: (name) @class.name) @class.def

; Interface declarations
(interface_declaration
  name: (name) @class.name) @class.def

; Trait declarations
(trait_declaration
  name: (name) @class.name) @class.def

; Base clause (extends)
(base_clause
  (name) @class.bases)

; Function calls
(function_call_expression
  function: (name) @call.name) @call.def

; Method calls
(member_call_expression
  name: (name) @method_call.name) @method_call.def

; Use declarations (imports)
(namespace_use_clause
  (namespace_use_group_clause
    (namespace_name) @import.module)) @import.def

(namespace_use_clause
  (namespace_name) @import.module) @import.def
