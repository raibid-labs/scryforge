# Scryforge Keybindings Reference

This document provides a comprehensive reference for all keyboard shortcuts available in the Scryforge TUI.

## Table of Contents

- [Philosophy](#philosophy)
- [Global Keys](#global-keys)
- [Navigation Keys](#navigation-keys)
- [Item Actions](#item-actions)
- [Omnibar Keys](#omnibar-keys)
- [Collection Picker Keys](#collection-picker-keys)
- [Key Reference Table](#key-reference-table)
- [Customization](#customization)

## Philosophy

Scryforge follows a vim-inspired keyboard navigation model:
- **Modal operation**: Different keys behave differently depending on context (normal mode, omnibar mode, etc.)
- **Efficient movement**: Home row keys (`hjkl`) for navigation
- **Mnemonic shortcuts**: Actions use memorable letters (e.g., `r` for read, `s` for save)
- **Arrow key support**: Standard arrow keys work alongside vim keys

## Global Keys

These keys work in any context and mode.

| Key | Action | Description |
|-----|--------|-------------|
| `q` | Quit | Exit the application (in normal mode only) |
| `?` | Help | Show abbreviated help text in status bar |
| `Esc` | Cancel | Close omnibar/picker, return to normal mode |

## Navigation Keys

### Pane Navigation

Switch focus between the three main panes (streams, items, preview).

| Key | Action | Direction |
|-----|--------|-----------|
| `Tab` | Next pane | Move focus right (streams → items → preview → streams) |
| `Shift+Tab` | Previous pane | Move focus left (preview → items → streams → preview) |
| `l` | Next pane | Move focus right (vim-style) |
| `h` | Previous pane | Move focus left (vim-style) |

**Pane order**: StreamList → ItemList → Preview → (back to StreamList)

### List Navigation

Navigate within the currently focused list (streams or items).

| Key | Action | Description |
|-----|--------|-------------|
| `j` | Move down | Select next item in list |
| `k` | Move up | Select previous item in list |
| `↓` | Move down | Arrow key alternative |
| `↑` | Move up | Arrow key alternative |
| `g` | Jump to first | Select first item in list |
| `G` | Jump to last | Select last item in list |

**Note**: List navigation only works when focused on the streams or items pane. The preview pane is read-only.

### Auto-Scrolling

- When navigating items, the preview pane automatically updates to show the selected item
- When changing streams, the item list automatically loads items from the new stream
- Items are automatically marked as read when selected

## Item Actions

These actions operate on the currently selected item. You must be focused on the **ItemList** pane for these to work (with the exception of viewing help).

| Key | Action | Description | Requirements |
|-----|--------|-------------|--------------|
| `Enter` | Open item | Open the selected item (not yet implemented) | Focus on ItemList |
| `r` | Toggle read status | Mark item as read/unread | Focus on ItemList |
| `s` | Toggle save status | Save/unsave the item | Focus on ItemList |
| `e` | Archive item | Archive item and remove from view | Focus on ItemList |
| `a` | Add to collection | Show collection picker to add item | Focus on ItemList |
| `d` | Remove from collection | Remove item from current collection (if viewing collection) | Focus on ItemList |

### Action Feedback

- All actions provide immediate visual feedback in the status bar
- Actions update the local UI immediately before syncing with the daemon
- Error messages appear in the status bar if an action fails

### Read/Unread Behavior

- Items are **automatically** marked as read when you navigate to them
- Press `r` to **manually** toggle read/unread status
- Unread items typically show a bullet or indicator in the item list

### Save Behavior

- Press `s` to toggle between saved and unsaved
- Saved items can be viewed across all streams via the "Saved" unified view
- Saved status is independent of read status

### Archive Behavior

- Press `e` to archive the current item
- Archived items are removed from the current view
- Archived items may still be accessible in an "Archive" stream (provider-dependent)

## Omnibar Keys

The omnibar is activated by pressing `/` (search) or `:` (commands). While the omnibar is active:

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Cancel | Close omnibar and discard input |
| `Enter` | Execute | Execute the command or search |
| `Backspace` | Delete character | Remove the last character |
| Any character | Type | Add character to input |

### Omnibar Modes

**Search mode** (activated with `/`):
- Immediately start typing your search query
- No prefix character in the input
- Press `Enter` to execute search

**Command mode** (activated with `:`):
- Input starts with `:` prefix
- Type command name after the colon
- Autocomplete suggestions appear as you type
- Press `Enter` to execute command

See [COMMANDS.md](COMMANDS.md) for complete search and command syntax.

## Collection Picker Keys

The collection picker appears when you press `a` to add an item to a collection.

| Key | Action | Description |
|-----|--------|-------------|
| `j` | Move down | Select next collection |
| `k` | Move up | Select previous collection |
| `↓` | Move down | Arrow key alternative |
| `↑` | Move up | Arrow key alternative |
| `Enter` | Confirm | Add item to selected collection |
| `Esc` | Cancel | Close picker without adding |

**Note**: If no collections exist, the first press of `a` will fetch collections from the daemon.

## Key Reference Table

Complete alphabetical listing of all keys and their functions:

| Key | Context | Action |
|-----|---------|--------|
| `?` | Global | Show help in status bar |
| `a` | ItemList (focused) | Add item to collection |
| `d` | ItemList (focused) | Remove item from collection |
| `e` | ItemList (focused) | Archive selected item |
| `g` | StreamList/ItemList (focused) | Jump to first item |
| `G` | StreamList/ItemList (focused) | Jump to last item |
| `h` | Normal mode | Move focus to previous pane (left) |
| `j` | StreamList/ItemList/Picker (focused) | Move down one item |
| `k` | StreamList/ItemList/Picker (focused) | Move up one item |
| `l` | Normal mode | Move focus to next pane (right) |
| `q` | Normal mode | Quit application |
| `r` | ItemList (focused) | Toggle read/unread status |
| `s` | ItemList (focused) | Toggle save/unsave status |
| `/` | Normal mode | Activate omnibar (search mode) |
| `:` | Normal mode | Activate omnibar (command mode) |
| `Enter` | Omnibar/Picker | Execute/Confirm |
| `Esc` | Omnibar/Picker | Cancel/Close |
| `Tab` | Normal mode | Move focus to next pane |
| `Shift+Tab` | Normal mode | Move focus to previous pane |
| `↑` | StreamList/ItemList/Picker (focused) | Move up one item |
| `↓` | StreamList/ItemList/Picker (focused) | Move down one item |
| `Backspace` | Omnibar | Delete last character |

## Context-Sensitive Behavior

Many keys behave differently depending on which pane is focused:

### When StreamList is Focused

- `j`/`k`: Navigate streams
- `g`/`G`: Jump to first/last stream
- Changing stream automatically loads items from that stream

### When ItemList is Focused

- `j`/`k`: Navigate items
- `g`/`G`: Jump to first/last item
- `r`: Toggle read/unread
- `s`: Toggle save/unsave
- `e`: Archive item
- `a`: Add to collection
- `d`: Remove from collection
- Changing items automatically updates preview

### When Preview is Focused

- Preview pane is read-only
- `j`/`k` have no effect
- Only pane switching keys work

### When Omnibar is Active

- Most navigation keys are disabled
- Character keys type into the omnibar
- Only `Enter` and `Esc` perform actions

### When Collection Picker is Active

- `j`/`k`: Navigate collections
- `Enter`: Add item to selected collection
- `Esc`: Cancel without adding
- Other keys are disabled

## Customization

**Note**: Custom keybindings are planned but not yet implemented. Currently, keybindings are hard-coded.

Future versions will support customizable keybindings via the configuration file:

```toml
# Future feature (not yet implemented)
[keybindings]
quit = "q"
search = "/"
toggle_read = "r"
# ... etc
```

## Learning Tips

### For Vim Users

If you're familiar with vim, you'll feel right at home:
- `hjkl` for directional movement
- `gg` and `G` for jumping to extremes (note: in Scryforge it's single `g` and `G`)
- `/` for search
- `:` for commands
- `Esc` to exit modes

### For Non-Vim Users

If you're new to vim-style navigation:
- Start with arrow keys and `Tab` - they work just like you'd expect
- Gradually try `j` and `k` instead of arrow keys
- Once comfortable, try `h` and `l` for pane switching
- The home row position minimizes hand movement and increases efficiency

### Quick Start Cheat Sheet

For your first session, remember these essentials:

1. **Navigation**: Use `Tab` and arrow keys
2. **Search**: Press `/`, type, press `Enter`
3. **Commands**: Press `:`, type command, press `Enter`
4. **Help**: Press `?` to see help in status bar
5. **Quit**: Press `q`

Everything else you can learn gradually!

## Accessibility Notes

### Alternative Input Methods

- All vim-style keys have arrow key equivalents
- `Tab` can be used instead of `h`/`l` for pane navigation
- Commands can be typed fully instead of using shortcuts

### Screen Readers

Scryforge is a terminal application that should work with screen readers that support terminal output. Key considerations:

- Status bar provides feedback for all actions
- Error messages appear in the status bar
- Current pane focus is indicated visually

## Troubleshooting

### Key doesn't work

- **Check focus**: Many keys only work when the correct pane is focused
- **Check mode**: Are you in omnibar mode? Press `Esc` to return to normal mode
- **Check status bar**: Status messages will indicate if an action is not available

### Unexpected behavior

- **Terminal conflicts**: Some terminals capture certain key combinations (e.g., `Ctrl+S` for flow control)
- **Shift key**: `Shift+Tab` vs `Tab`, `G` vs `g` - case matters for some keys
- **Numeric keypad**: Use the main keyboard numbers, not the numeric keypad

### Key combinations not working

Currently, Scryforge uses single keys only. No `Ctrl+`, `Alt+`, or multi-key sequences (except `Shift+Tab` and `Shift+G`).

## See Also

- [COMMANDS.md](COMMANDS.md) - Complete omnibar command reference
- [GETTING_STARTED.md](GETTING_STARTED.md) - Basic usage guide
- [ARCHITECTURE.md](ARCHITECTURE.md) - Understanding the TUI architecture
