#pragma once

// The list's other combat mods, told to step aside while Elden Ring mode is
// on, through their own modder APIs when they are installed:
// - True Directional Movement: its target lock is switched off (ours
//   replaces it), the player's yaw is ours during rolls, attacks and jumps and
//   while our lock-on holds a target, and its directional movement is paused
//   while locked on (Elden Ring strafes round the target).
// - TrueHUD: shows its lock-on marker on our target.
// Off again, everything is released and the list plays as before.
namespace Integrations
{
    // Once all plugins are loaded.
    void Load();

    // Elden Ring combat mode switched on or off.
    void SetErMode(bool on);

    // Each frame in Elden Ring mode: whether the core owns the player's yaw
    // (and the heading it wants), whether TDM's directional movement must
    // pause, and the lock-on target (null: none).
    void Update(bool ownYaw, float heading, bool pauseDirectional, RE::Actor* target);
}
