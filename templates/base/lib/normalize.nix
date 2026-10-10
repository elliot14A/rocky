serviceList:
builtins.foldl' (
  acc: entry: if builtins.isString entry then acc // { ${entry} = { }; } else acc // entry
) { } serviceList
