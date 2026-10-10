#include "Input.h"

#include "Settings.h"

Input* Input::Get()
{
    static Input input;
    return &input;
}

void Input::Register()
{
    if (auto* devices = RE::BSInputDeviceManager::GetSingleton()) {
        devices->AddEventSink<RE::InputEvent*>(this);
    }
}

ErInput Input::Take(float cameraHeading)
{
    ErInput out{};
    // Keyboard movement wins over a resting stick.
    float x = static_cast<float>(right) - static_cast<float>(left);
    float y = static_cast<float>(forward) - static_cast<float>(back);
    if (x == 0.0f && y == 0.0f) {
        x = stickX;
        y = stickY;
    }
    const float length = std::sqrt(x * x + y * y);
    if (length > 1.0f) {
        x /= length;
        y /= length;
    }
    // Walking on the keyboard: a light tilt.
    if (walk && length > 0.0f) {
        x *= 0.4f;
        y *= 0.4f;
    }
    out.move_x = x;
    out.move_y = y;
    out.camera_heading = cameraHeading;
    out.dodge = dodge.Take();
    out.jump = jump.Take();
    out.light = light.Take();
    out.heavy = heavy.Take();
    out.guard = guard.Take();
    out.walk = walk;
    return out;
}

RE::BSEventNotifyControl Input::ProcessEvent(RE::InputEvent* const* a_event, RE::BSTEventSource<RE::InputEvent*>*)
{
    if (!a_event) {
        return RE::BSEventNotifyControl::kContinue;
    }
    const auto& settings = Settings::Get();
    const auto* userEvents = RE::UserEvents::GetSingleton();

    for (auto* event = *a_event; event; event = event->next) {
        if (event->GetEventType() == RE::INPUT_EVENT_TYPE::kThumbstick) {
            if (auto* stick = static_cast<RE::ThumbstickEvent*>(event); stick->IsLeft()) {
                stickX = stick->xValue;
                stickY = stick->yValue;
            }
            continue;
        }
        auto* button = event->AsButtonEvent();
        if (!button) {
            continue;
        }
        const bool down = button->IsPressed();
        const auto& user = button->QUserEvent();
        if (user == userEvents->forward) {
            forward = down;
        } else if (user == userEvents->back) {
            back = down;
        } else if (user == userEvents->strafeLeft) {
            left = down;
        } else if (user == userEvents->strafeRight) {
            right = down;
        } else if (user == userEvents->activate && button->IsDown()) {
            activatePressed = true;
        }

        // Our own keys, by scan code; mouse buttons are offset by 256.
        std::uint32_t code = button->GetIDCode();
        if (button->GetDevice() == RE::INPUT_DEVICE::kMouse) {
            code += 256;
        } else if (button->GetDevice() != RE::INPUT_DEVICE::kKeyboard) {
            continue;  // TODO: gamepad bindings.
        }

        if (code == settings.toggleKey) {
            // Not while typing in the console or a text field, or in menus.
            auto* ui = RE::UI::GetSingleton();
            if (button->IsDown() && ui && !ui->GameIsPaused() && !ui->IsMenuOpen(RE::Console::MENU_NAME)) {
                enabled = !enabled;
                toggled = true;
            }
        } else if (code == settings.dodgeKey) {
            dodge.Set(down);
        } else if (code == settings.jumpKey) {
            jump.Set(down);
        } else if (code == settings.heavyModifier) {
            heavyModifierHeld = down;
        } else if (code == settings.walkKey && button->IsDown()) {
            walk = !walk;
        } else if (code == settings.lightKey) {
            // Shift + light is the heavy attack, as in Elden Ring on keyboard.
            (heavyModifierHeld || heavy.held ? heavy : light).Set(down);
        } else if (code == settings.guardKey) {
            guard.Set(down);
        }
    }
    return RE::BSEventNotifyControl::kContinue;
}
