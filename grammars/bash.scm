; Function definitions
(function_definition
  name: (word) @func.name) @func.def

; Command calls
(command
  name: (command_name
    (word) @call.name)) @call.def

; Source/dot includes
(command
  name: (command_name
    (word) @_cmd)
  argument: (word) @import.module
  (#match? @_cmd "^(source|\\.)$")) @import.def
