# Getting Started with Pantry

Pantry is a generic selector tool for handling various types of entries with text and image preview modes. This guide will help you get started with pantry and show you various usage patterns.

## Table of Contents

- [Basic Usage](#basic-usage)
- [Command Line Options](#command-line-options)
- [Configuration](#configuration)
- [Categories](#categories)
- [Preview Modes](#preview-modes)
- [Piping Input and Output](#piping-input-and-output)
- [Example Configurations](#example-configurations)

## Basic Usage

Basic usage of pantry:

```bash
pantry -f config.toml
```

This will open the pantry GUI with entries from the specified configuration file.

## Command Line Options

Pantry supports several command line options:

- `-f, --config`: Configuration file path [default: `~/.config/pantry/config.toml`]
- `-c, --category`: Specify the category to load (load only categories matching the global display mode if not specified)
- `-d, --display`: Display mode: text or picture (overrides config file setting)

## Configuration

Pantry uses TOML format configuration files with separate display and input modes. The configuration contains global defaults and entries for various categories. Each category can optionally specify its own modes, which will override the global defaults.

The new configuration format separates entries from category settings using a `.entries` sub-table:

```toml
# Global default display mode
display = "text"

# Commands category - uses default "text" display mode
[commands]
display = "text"

[commands.entries]
"Shutdown" = "shutdown now"
"Reboot" = "reboot"

# Live wallpapers category - explicitly set to "picture" display mode
[live]
display = "picture"

[live.entries]
"live" = "~/Pictures/wallpapers/ja/"
```

## Categories

You can specify a specific category to load using the `-c` option:

```bash
pantry -f config.toml -c bookmarks
```

This will load only entries from the "bookmarks" category.

When no category is specified with `-c`, pantry will load only categories that match the global default display mode. For example, if the global display mode is set to "text", only categories with display mode "text" will be loaded, and categories with display mode "picture" will be ignored. This helps keep the interface clean and relevant to the selected display mode.

## Preview Modes

Pantry supports two display modes:

- `text` mode: For text entries like bookmarks, commands, etc.
- `picture` mode: For image files with preview functionality

The display mode can be set globally, per category, or overridden with the `-d` command line option.

## Piping Input and Output

Pantry now supports both input and output piping, making it more flexible and Unix-like:

### Input Piping

You can pipe data directly to pantry without using a configuration file:

```bash
echo -e "Option 1\nOption 2\nOption 3" | pantry
```

You can also specify the display mode when using piped input:

```bash
ls ~/Pictures/ | pantry -d picture
```

### Output Piping

One of pantry's powerful features is the ability to pipe its output to other commands. When you select an entry in pantry and press Enter, the value of that entry is output to stdout, which can then be piped to other commands.

#### Examples:

Open selected URL in your default browser:

```bash
pantry -f example-bookmarks.toml | xargs xdg-open
```

Execute selected command:

```bash
pantry -f example-commands.toml | xargs sh
```

Copy selected entry to clipboard (using xclip):

```bash
pantry -f example-bookmarks.toml | xargs xclip -selection clipboard
```

Set selected image as wallpaper:

```bash
pantry -f example-pictures.toml | xargs nitrogen --set-zoom-fill
```

SSH to selected server:

```bash
pantry -f servers.toml | xargs ssh
```

Navigate to selected directory:

```bash
cd "$(pantry -f directories.toml)"
```

Open selected file with default application:

```bash
pantry -f files.toml | xargs xdg-open
```

Run selected script:

```bash
pantry -f scripts.toml | xargs chmod +x && pantry -f scripts.toml | xargs bash
```

Use with fzf for additional filtering:

```bash
pantry -f bookmarks.toml | fzf | xargs xdg-open
```

## Example Configurations

Pantry comes with several example configuration files to help you get started:

- [example-bookmarks.toml](example-bookmarks.toml) - Example configuration for managing bookmarks and URLs
- [example-commands.toml](example-commands.toml) - Example configuration for system commands
- [example-pictures.toml](example-pictures.toml) - Example configuration for image collections

These examples demonstrate the new configuration format with separated entries and the category display mode override feature.