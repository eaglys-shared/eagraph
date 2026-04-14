; Function definitions (def/defp)
(call
  target: (identifier) @_keyword
  (arguments
    (call
      target: (identifier) @func.name))
  (#match? @_keyword "^defp?$")) @func.def

; Module definitions
(call
  target: (identifier) @_keyword
  (arguments
    (alias) @class.name)
  (#eq? @_keyword "defmodule")) @class.def

; Function calls
(call
  target: (identifier) @call.name) @call.def

; Remote calls (Module.function)
(call
  target: (dot
    left: (_) @method_call.object
    right: (identifier) @method_call.name)) @method_call.def

; Alias (import/require/use)
(call
  target: (identifier) @_keyword
  (arguments
    (alias) @import.module)
  (#match? @_keyword "^(alias|import|require|use)$")) @import.def
