# The types of services.yokoso.settings and of a theme, from the JSON
# schemas in shell/schema, so that they never go apart. A key left unset
# stays out of the JSON (yokoso's default holds), and one yokoso doesn't
# know goes in as it is (yokoso names it in the journal). A value made of
# options carries `_out`, what was set of it, for the JSON.
{ lib, json }:

let
  inherit (lib) types;
  theme = lib.importJSON ../shell/schema/theme.json;
  config = lib.importJSON ../shell/schema/config.json;
  defs = theme."$defs" // config."$defs";

  resolve =
    s:
    if s ? "$ref" then defs.${lib.removePrefix "#/$defs/" s."$ref"} // removeAttrs s [ "$ref" ] else s;

  # A picture or a QML file: a path in the Nix config, copied to the store.
  file = types.coercedTo types.path (p: "${p}") types.str;

  # A schema as a type; `isFile` for the files' keys.
  toType =
    isFile: s0:
    let
      s = resolve s0;
    in
    if s ? enum then
      types.enum s.enum
    else if s ? oneOf then
      types.oneOf (map (toType isFile) s.oneOf)
    else if lib.isList (s.type or null) then
      (if lib.elem "null" s.type then types.nullOr else lib.id) (
        toType isFile (s // { type = lib.head (lib.remove "null" s.type); })
      )
    else
      {
        string = if isFile then file else types.str;
        boolean = types.bool;
        number = types.number;
        integer = types.int;
        array = types.listOf (if s ? items then toType isFile s.items else json.type);
        object =
          if s ? properties then
            object s
          else
            types.attrsOf (
              if lib.isAttrs (s.additionalProperties or null) then
                toType isFile s.additionalProperties
              else
                json.type
            );
      }
      .${s.type or ""} or json.type;

  out =
    v:
    if lib.isAttrs v && v ? _out then
      v._out
    else if lib.isList v then
      map out v
    else
      v;

  object =
    s:
    let
      properties = lib.filterAttrs (name: _: !lib.hasPrefix "$" name) s.properties;
      required = s.required or [ ];
      extra = s.additionalProperties or true;
    in
    types.submodule (
      { options, config, ... }:
      {
        freeformType =
          if extra == false then
            null
          else if lib.isAttrs extra then
            types.attrsOf (toType false extra)
          else
            json.type;
        options =
          lib.mapAttrs (
            name: p:
            lib.mkOption (
              {
                type = toType (lib.elem name [
                  "background"
                  "look"
                ]) p;
              }
              // lib.optionalAttrs (resolve p ? description) { inherit (resolve p) description; }
            )
          ) properties
          // {
            _out = lib.mkOption {
              type = types.raw;
              internal = true;
              readOnly = true;
            };
          };
        config._out = lib.mapAttrs (_: out) (
          lib.filterAttrs (
            name: _:
            !lib.hasPrefix "_" name
            && (!properties ? ${name} || lib.elem name required || options.${name}.isDefined)
          ) config
        );
      }
    );
in
{
  inherit out;
  settings = object (
    config
    // {
      properties = config.properties // removeAttrs theme.properties [ "extends" ];
      additionalProperties = true;
    }
  );
  theme = object theme;
}
