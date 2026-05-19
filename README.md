# Pixel Pet

Pixel Pet is a lightweight macOS-first desktop companion: part electronic pet, part gentle break reminder, and part tiny piece of pixel art that belongs to you.

The first version is intentionally small. When you start Pixel Pet for the first time, you draw your own pixel pet. From there, it lives quietly on your desktop, reacts to your work rhythm, and reminds you to take care of yourself through simple visual states instead of popups, alarms, or productivity pressure.

Pixel Pet is currently in the early planning stage.

## Product Idea

Most break reminder tools feel like timers. Pixel Pet should feel more like a little companion.

It does not need to show how long you have been working. It does not need to measure your productivity. It should simply notice broad activity patterns, change its state, and make rest feel a little more natural.

The core experience is:

1. Open Pixel Pet.
2. Draw your own pet in a minimal pixel editor.
3. Let it stay on your desktop.
4. Watch it react as you work, pause, eat, or disappear for a while.

## MVP Scope

The MVP focuses on a small, local, macOS-first experience.

### Core Features

- First-run pixel pet drawing flow
- Minimal pixel editor with brush, eraser, color selection, and save
- Adjustable desktop pet size, visually around a little larger than a folder icon by default
- Lightweight desktop companion window
- Local activity detection based on mouse and keyboard activity events
- Rule-based pet state changes
- Built-in default pet as a fallback, demo, and possible app icon

### Pet States

The first version should support these states:

- `startup greeting`: the pet appears or wakes up when the app starts
- `working`: the user is actively using the computer
- `stretch`: the main break-reminder state after roughly an hour of activity
- `tired`: the pet looks exhausted after several hours of continued activity
- `sleep`: the pet sleeps after a long period without activity
- `meal`: the pet reacts around meal times

The MVP should communicate through visual state changes only. No system notifications, speech bubbles, or intrusive alerts are required for the first version.

## Pixel Editor Philosophy

Pixel Pet should provide a simple creative environment without making the user feel boxed in.

The editor should be minimal, but not limiting. Some users may only want to draw a few pixels. Others may create something surprisingly expressive with the same small set of tools. The product should respect both.

For the MVP, the editor should avoid becoming a full art program. The goal is to help users make a personal pet quickly, not to build a professional sprite editor.

## Animation Approach

The MVP uses rule-based animation.

Pixel Pet does not need to understand the drawing semantically in the first version. It can create states through lightweight transformations such as stretching, squashing, shifting, breathing, or adding simple visual props.

If a user draws an incomplete pet, Pixel Pet may use simple template-based completion while preserving the original drawing as the source of truth.

The first version should not promise perfect AI-generated animation.

## Privacy Principles

Pixel Pet should be local-first and privacy-respecting.

- No cloud account
- No analytics
- No telemetry
- No uploading user behavior
- No recording raw keystrokes
- No capturing screen content
- No tracking productivity metrics

Activity detection should only answer a simple question: has the user been active recently?

Depending on the final macOS implementation, Pixel Pet may require limited system permissions for activity detection. These permissions should be explained clearly and kept as narrow as possible.

## Technology Direction

The preferred direction for the first implementation is:

- macOS first
- Tauri + Rust + lightweight web UI
- Low memory usage
- Low CPU usage
- Minimal background work
- No heavy runtime unless there is a clear reason

Auto-start is a planned feature, but it does not need to be part of the earliest MVP milestone.

## Roadmap

Possible future improvements include:

- Auto-start on login
- Windows and Linux support
- PNG import and export
- Manual editing for each pet state
- Richer pixel editor tools
- More animation presets
- Optional AI-assisted sprite completion, only if it preserves the privacy-first and lightweight goals of the project

## Non-Goals

Pixel Pet is not meant to be:

- A full productivity tracker
- A time-tracking app
- A habit scoring system
- A cloud-connected pet platform
- A heavy desktop widget framework
- A professional pixel art editor

The pet should support the user’s workday without turning the workday into data.

## License

MIT
