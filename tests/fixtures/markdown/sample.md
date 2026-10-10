# Sample Document

This is an introduction paragraph with a [[FirstNote]] link.

## Section With Code

Here is some code:
```rust
fn example() {
    let _wiki = "[[CodeLink]]";
    let _md = "[CodeMd](https://code.example.com)";
}
```

And inline code `[[InlineCodeLink]]` should also be ignored.

Also escaped link \[[EscapedLink]] should not be detected.

Here is a duplicate link: [[FirstNote]] again and [[FirstNote#Heading|Custom Alias]].

A paragraph with a block ID. ^block-123
