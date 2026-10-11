#pragma once

#include "er_combat.h"

// Listens to Skyrim's raw input and turns it into the core's ErInput.
// Movement comes from the remappable Forward/Back/Strafe user events and the
// left stick, so it follows the player's own bindings.
class Input : public RE::BSTEventSink<RE::InputEvent*>
{
public:
    static Input* Get();
    void Register();

    // The controls since the last call. Clears the one-frame edges.
    ErInput Take(float cameraHeading);

    // Activate was pressed since the last call (for the bridge to decide
    // whether it may go through).
    bool activatePressed = false;

    // Lock-on was pressed since the last call (middle mouse / R3 by default).
    bool lockPressed = false;

    // Elden Ring mode, flipped by the toggle key. Off at startup.
    bool enabled = false;
    bool toggled = false;  // flipped since the bridge last looked

protected:
    RE::BSEventNotifyControl ProcessEvent(RE::InputEvent* const* a_event, RE::BSTEventSource<RE::InputEvent*>*) override;

private:
    struct Key
    {
        bool held = false;
        bool pressed = false;
        bool released = false;

        void Set(bool down)
        {
            if (down && !held) {
                pressed = true;
            }
            if (!down && held) {
                released = true;
            }
            held = down;
        }

        ErButton Take()
        {
            ErButton b{ static_cast<std::uint8_t>(held), static_cast<std::uint8_t>(pressed), static_cast<std::uint8_t>(released) };
            pressed = released = false;
            return b;
        }
    };

    Key dodge, jump, light, heavy, guard;
    bool heavyModifierHeld = false;
    bool crouchPressed = false;
    bool walk = false;
    bool forward = false, back = false, left = false, right = false;
    float stickX = 0.0f, stickY = 0.0f;
};
