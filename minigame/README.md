# Minigame room service

Minigame is a native extension that provides `minigame:rooms/v1`. It manages
a temporary domain with a lobby and an arena, accepts human players up to a
configured capacity, tracks bots, and restores players to their original
domains before retiring the worlds.

Build and install the package from the Steel checkout.

```sh
python3 package-extension.py extension-example/minigame
```

Steel discovers `extensions/steel-minigame-extension.zip` on startup.
[Spleef](../spleef/README.md) is a consumer.

To use the service in another extension, add the `minigame-api` Cargo
dependency and declare `depends: ["minigame"]` in `export_extension!`.
Resolve `minigame_api::rooms()` during init. Give `Rooms::create` your
registered generator keys, world settings, and room capacity. The first
successful `Room::join` requests the lobby.

Drive `Room::tick` once per server tick with the current player snapshot.
Start the arena with `Room::start_arena` when your game is ready, then use
`Room::enter_arena` to transfer your roster and detect when players finish
loading. The consumer decides countdowns, equipment, scoring, and win
conditions. Call `Room::close` at the end and keep ticking until
`RoomPhase::Closed` so pending operations, player returns, and world cleanup
can finish. Rooms also close when every human leaves or disconnects.
