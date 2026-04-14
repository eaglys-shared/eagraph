; Function definitions
(function
  name: (variable) @func.name) @func.def

; Type declarations
(type_alias
  name: (type) @class.name) @class.def

; Data declarations
(data_type
  name: (type) @class.name) @class.def

; Newtype declarations
(newtype
  name: (type) @class.name) @class.def

; Class declarations
(class
  name: (type) @class.name) @class.def

; Instance declarations
(instance
  name: (type) @class.name) @class.def

; Function applications (calls)
(apply
  (variable) @call.name) @call.def

; Import statements
(import
  module: (module) @import.module) @import.def
