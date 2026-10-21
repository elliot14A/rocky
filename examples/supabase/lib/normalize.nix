services:
if services == null then
  { }
else if builtins.isAttrs services then
  services
else
  builtins.foldl' (
    acc: entry:
    if entry == null then
      acc
    else if builtins.isString entry then
      acc // { ${entry} = { }; }
    else
      acc // entry
  ) { } services
