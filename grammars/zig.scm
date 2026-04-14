; Function declarations
(FnProto
  name: (IDENTIFIER) @func.name) @func.def

; Struct declarations
(ContainerDecl) @class.def

; Const declarations (used for type aliases and namespaces)
(VarDecl
  name: (IDENTIFIER) @func.name) @func.def

; Function calls
(FnCallExpr
  (IDENTIFIER) @call.name) @call.def

; Field access calls
(FieldAccess
  (IDENTIFIER) @method_call.name) @method_call.def

; @import builtin
(BuiltinCallExpr
  (IDENTIFIER) @_fn
  (FnCallArguments
    (STRINGLITERALSINGLE) @import.module)
  (#eq? @_fn "@import")) @import.def
