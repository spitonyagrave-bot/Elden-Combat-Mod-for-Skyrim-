#pragma once

// Elden Ring's own movement and camera while Elden Ring combat mode is on
// (ERCombat.ini [Movement] Enabled=1):
// - The camera orbits freely around the player (Skyrim's sheathed-weapon
//   free rotation, kept on with the weapon drawn), so turning the mouse or
//   the right stick no longer drags the character round with it.
// - The core turns the player toward the stick at Elden Ring's turn rates;
//   the camera is held where it is while the player turns under it.
// - Skyrim's movement input is rewritten so the player runs where the core
//   says (forward along the facing, or strafing round a lock-on target), and
//   Skyrim's character controller does the walking: slopes, stairs, water
//   and collision stay Skyrim's.
// - Locked on, the camera glides toward the target instead of snapping.
// - The right stick's look input eases in, as Elden Ring's camera does.
// It also keeps Skyrim from acting on the gamepad buttons the core uses
// (B opening the tween menu, the shoulder buttons shouting).
class Movement
{
public:
    static void Install();

    // Elden Ring owns movement and camera from now on (or hands them back).
    static void SetActive(bool on);
    static bool Active() { return active; }

    // Skyrim's movement input for the next frames, in the player's own frame
    // (x right, y forward, length 0..1).
    static void SetMove(float x, float y);

    // Turns the player to `heading`, keeping the camera where it is.
    static void Turn(RE::PlayerCharacter* player, float heading);

    // Glides the camera toward looking along `yaw` (the lock-on target).
    static void SteerCamera(RE::PlayerCharacter* player, float yaw, float delta);

    // Locked on: sideways mouse or right-stick movement no longer turns the
    // camera; a flick switches targets instead (Elden Ring's lock switch).
    static void SetLocked(bool on);
    // -1 (left), +1 (right) once per flick, else 0.
    static int TakeFlick();

    // The camera's heading (the player's facing plus the free rotation).
    static float CameraHeading(RE::PlayerCharacter* player);

private:
    static RE::ThirdPersonState* ThirdPerson();

    static void ProcessThumbstick(RE::MovementHandler* a_this, RE::ThumbstickEvent* a_event, RE::PlayerControlsData* a_data);
    static void ProcessButton(RE::MovementHandler* a_this, RE::ButtonEvent* a_event, RE::PlayerControlsData* a_data);
    static inline REL::Relocation<decltype(ProcessThumbstick)> _ProcessThumbstick;
    static inline REL::Relocation<decltype(ProcessButton)> _ProcessButton;

    static void LookThumbstick(RE::LookHandler* a_this, RE::ThumbstickEvent* a_event, RE::PlayerControlsData* a_data);
    static inline REL::Relocation<decltype(LookThumbstick)> _LookThumbstick;

    static void LookMouse(RE::LookHandler* a_this, RE::MouseMoveEvent* a_event, RE::PlayerControlsData* a_data);
    static inline REL::Relocation<decltype(LookMouse)> _LookMouse;

    static void SetFreeRotationMode(RE::ThirdPersonState* a_this, bool a_weaponSheathed);
    static inline REL::Relocation<decltype(SetFreeRotationMode)> _SetFreeRotationMode;

    static bool ShoutCanProcess(RE::ShoutHandler* a_this, RE::InputEvent* a_event);
    static inline REL::Relocation<decltype(ShoutCanProcess)> _ShoutCanProcess;
    static bool MenuOpenCanProcess(RE::MenuOpenHandler* a_this, RE::InputEvent* a_event);
    static inline REL::Relocation<decltype(MenuOpenCanProcess)> _MenuOpenCanProcess;

    static inline std::atomic<bool> active = false;
    static inline RE::NiPoint2 move{ 0.0f, 0.0f };
    static inline float lookX = 0.0f, lookY = 0.0f;
    static inline std::chrono::steady_clock::time_point lastLook{};
    static inline std::atomic<bool> locked = false;
    static inline std::atomic<int> flick = 0;
    static inline float flickSum = 0.0f;
    static inline std::chrono::steady_clock::time_point lastFlickInput{};
    static inline std::chrono::steady_clock::time_point flickBlockedUntil{};
    static inline bool stickCentred = true;
};
