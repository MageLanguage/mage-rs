## Usage

```
[gohryt@pc ~]$ cast --help
Usage: cast [OPTIONS] <PATH>

Arguments:
  <PATH>

Options:
      --stage <STAGE>    [default: execute] [possible values: load, flatten, compile, execute]
      --output <OUTPUT>  [default: text] [possible values: text, json]
      --format <FORMAT>  [default: pretty] [possible values: pretty, simple]
      --save
  -h, --help             Print help
```

## Code style

- Use full words for variable names, not abbreviations (`expression` not `expr`).
- Comments only for non-obvious behavior. Do not restate what code already says.
- Test file names follow the pattern `{module}_test.rs`, registered via `#[cfg(test)] mod {module}_test;`.
