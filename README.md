# SimpleUnc

On Windows,
[`fs::canonicalize`] returns a path prefixed by "`\\?\`".
such as:
```
\\?\UNC\server\share\dir
```
The "`\\?\`" prefix is called the [Win32 File Namespaces].
It has advantages such as long paths,
and is fine for most modern APIs,
but some programs can't handle them.
PowerShell and `cmd.exe` are examples of such programs.

The `SimpleUnc` simplifies network share UNC paths
so that these programs can handle.

Since the simplification may not always be safe,
it does so if they are currently mapped to drives.

Let's say your PC has a network share on the `Z:` drive:
```
net use Z: \\server\share
```
If you run the following code on this PC:
```rust
let path = r"Z:\dir\file";
let canonicalized = fs::canonicalize(path)?;
println!("{}", canonicalized.display());
```
prints:
```
\\?\UNC\server\share\dir\file
```
Neither PowerShell nor `cmd.exe` can handle this path.

With the `SimpleUnc`:
```rust
let path = r"Z:\dir\file";
let simplified = SimpleUnc::default().canonicalize(path)?;
println!("{}", simplified.display());
```
prints:
```
\\server\share\dir\file
```
This path works fine for PowerShell and `cmd.exe`.

## Drive

If you prefer to use the network drive in the path:
```rust
let path = r"Z:\dir\file";
let unc = SimpleUnc { map_to_drive: true, ..Default::default() };
let simplified = unc.canonicalize(path)?;
println!("{}", simplified.display());
```
prints:
```
Z:\dir\file
```

## Dunce

The `SimpleUnc` calls the [`dunce`] crate
to normalize some other cases, such as:
```
\\?\C:\foo
```
to:
```
C:\foo
```

[`dunce`]: https://crates.io/crates/dunce
[`fs::canonicalize`]: https://doc.rust-lang.org/std/fs/fn.canonicalize.html
[Win32 File Namespaces]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#win32-file-namespaces
