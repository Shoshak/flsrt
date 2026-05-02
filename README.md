# flsrt

A work in progress CLI tool for sorting files. 

# Example

Default config directory: 
- Windows - `C:\Users\user\AppData\Roaming\flsrt`
- Linux - `/home/user/.config/flsrt`
- MacOS - `/Users/user/Library/Application Support/flsrt`

It consists of two directories: `rules` and `scripts`.

# Rules

Contains declarations of rule units.

rules/clips.json
```json
{
    "name": "Video",
    "description": "Sort videos by directories",
    "groups": ["Sort"],
    "paths": ["/home/motok/Videos", "/home/motok/Videos2"],
    "recursive": false,
    "script": "clips.lua"
}
```
# Scripts

Contains scripts that return actions to perform.

rules/clips.lua
```lua
local name = meta.file.name
local date, time, ext = string.match(name, "(%d%d%d%d%-%d%d%-%d%d) %- (%d%d%-%d%d%-%d%d)%.(.+)")

if ext ~= "mp4" and ext ~= "mov" and ext ~= "mkv" then
    return {}
end

local video_length = meta.video.length(meta.file.path)
if video_length < 15 or video_length > 60 then
    return {}
end

local out_name = string.format("Clip %s - %s.%s", date, time, ext)
return {
    copy = {"/home/motok/clips/" .. out_name, "/home/motok/backup/" .. out_name},
    move = "/home/motok/.local/share/Trash/" .. out_name
}
```
