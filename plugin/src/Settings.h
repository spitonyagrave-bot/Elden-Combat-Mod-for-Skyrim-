#pragma once

#include "er_combat.h"

// Data/SKSE/Plugins/ERCombat.ini. Read once at startup.
struct Settings
{
    // DirectInput scan codes for keyboard keys; mouse buttons are 256 + n
    // (256 left, 257 right, 258 middle), the same scheme SkyUI's MCM uses.
    std::uint32_t toggleKey = 0x28;     // Ä on a German keyboard (' on US): Elden Ring mode on/off
    std::uint32_t dodgeKey = 0x38;      // Left Alt: tap to roll, hold to sprint
    std::uint32_t jumpKey = 0x39;       // Space
    std::uint32_t lightKey = 256;       // Left mouse
    std::uint32_t guardKey = 257;       // Right mouse: guard, or left-hand attack
    std::uint32_t heavyModifier = 0x2A; // Left Shift + light = heavy
    std::uint32_t walkKey = 0x3A;       // Caps Lock
    std::uint32_t lockKey = 258;        // Middle mouse: lock on / off, as in Elden Ring
    std::uint32_t lockPad = 0x0080;     // Gamepad R3 (XInput right thumb)
    // Gamepad buttons (Skyrim's XInput codes), Elden Ring's layout.
    std::uint32_t padDodge = 0x2000;    // B: tap to roll, hold to sprint
    std::uint32_t padLight = 0x0200;    // RB: attack
    std::uint32_t padHeavy = 0x000A;    // RT: heavy attack, hold to charge
    std::uint32_t padGuard = 0x0100;    // LB: guard, or the left-hand attack

    // Skyrim animation event sent when each kind of action starts. Empty sends
    // nothing. Directional actions try "<event><Dir>" first (Forward, Back,
    // Left, Right), e.g. TK Dodge's TKDodgeForward.
    std::array<std::string, ER_ACTION_COUNT> events{};
    // Sent when the core raises and lowers the guard.
    std::string blockStart = "blockStart";
    std::string blockStop = "blockStop";

    // Let Precision, MCO and the rest swing the weapon (phase A), or let the
    // core's own hit detection decide what was hit (phase B).
    bool coreHitDetection = false;
    // While the pose driver runs: the core's root motion moves the player
    // during rolls, attacks and jumps; hits scale Skyrim's weapon damage.
    bool rootMotion = true;
    float damageScale = 1.0f;
    float hitShake = 0.3f;
    // Skyrim units added to a target's body radius for the hit test.
    float hitPadding = 20.0f;
    std::string hitSound;

    // [Pose]: play the Elden Ring animations on the player's skeleton
    // (needs ERCombat.eranims). Hook 1: after the player's animation update;
    // 0: after the player's update.
    bool poseDriver = true;
    std::int32_t poseHook = 1;
    // Per category, whose animation shows while the core runs the move:
    // Rolls=TKDodge (TK Dodge's directional rolls; the core still owns
    // timing, i-frames and stamina) or ER; Jumps=Skyrim (Skyrim's own jump,
    // which really leaves the ground) or ER.
    bool rollsSkyrim = true;
    bool jumpsSkyrim = true;

    // [Movement]: Elden Ring owns running, turning and the camera (free
    // camera, 360-degree movement). LockCameraSpeed: how fast the camera
    // glides onto a lock-on target (per second). PadLookSmoothing: seconds
    // the right stick's look takes to ease in (0 = raw).
    bool ownMovement = true;
    float lockCameraSpeed = 6.0f;
    float padLookSmoothing = 0.08f;

    // [Weapons] AutoTwoHand: a one-handed weapon with an empty left hand is
    // held in both hands (Elden Ring's two-handed stance, attacks and guard).
    bool autoTwoHand = true;

    // [LockOn], metres.
    float lockRange = 15.0f;
    float lockBreakRange = 22.0f;

    static Settings& Get();
    void Load();
};
