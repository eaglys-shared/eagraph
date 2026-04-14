; Method definitions
(method
  name: (identifier) @func.name) @func.def

; Singleton method definitions
(singleton_method
  name: (identifier) @func.name) @func.def

; Class definitions
(class
  name: (constant) @class.name) @class.def

; Superclass
(superclass
  (constant) @class.bases)

; Module definitions
(module
  name: (constant) @class.name) @class.def

; Method calls
(call
  method: (identifier) @call.name) @call.def

; Require calls
(call
  method: (identifier) @_req
  arguments: (argument_list (string) @import.module)
  (#match? @_req "^require"))  @import.def
