; Function definitions
(function_definition
  name: (identifier) @func.name
  body: (block) @func.body) @func.def

; Class definitions
(class_definition
  name: (identifier) @class.name
  superclasses: (argument_list)? @class.bases
  body: (block) @class.body) @class.def

; Import statements: import foo, import foo.bar
(import_statement
  name: (dotted_name) @import.module) @import.def

; From-import statements: from foo import bar
(import_from_statement
  module_name: (dotted_name)? @from_import.module) @from_import.def

; Function/method calls
(call
  function: (identifier) @call.name
  arguments: (argument_list) @call.args) @call.def

; Method calls: obj.method()
(call
  function: (attribute
    attribute: (identifier) @method_call.name)
  arguments: (argument_list) @method_call.args) @method_call.def
