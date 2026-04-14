; Method declarations
(method_declaration
  name: (identifier) @func.name) @func.def

; Constructor declarations
(constructor_declaration
  name: (identifier) @func.name) @func.def

; Class declarations
(class_declaration
  name: (identifier) @class.name) @class.def

; Interface declarations
(interface_declaration
  name: (identifier) @class.name) @class.def

; Enum declarations
(enum_declaration
  name: (identifier) @class.name) @class.def

; Superclass
(superclass
  (type_identifier) @class.bases)

; Super interfaces
(super_interfaces
  (type_list
    (type_identifier) @class.bases))

; Method invocations
(method_invocation
  name: (identifier) @method_call.name) @method_call.def

; Import declarations
(import_declaration
  (scoped_identifier) @import.module) @import.def
