#include "Movement.h"

#include "Input.h"
#include "Settings.h"

namespace
{
    constexpr float kPi = 3.14159265f;

    float Wrap(float a)
    {
        while (a > kPi) {
            a -= 2.0f * kPi;
        }
        while (a < -kPi) {
            a += 2.0f * kPi;
        }
        return a;
    }

    // Skyrim 1.7.99+ added two functions to PlayerInputHandler's vtable
    // ahead of the ones hooked here (the same shift TDM applies).
    std::size_t InputHandlerShift()
    {
        return REL::Module::get().version() >= REL::Version(1, 7, 99, 0) ? 2 : 0;
    }
}

void Movement::Install()
{
    const auto shift = InputHandlerShift();
    REL::Relocation<std::uintptr_t> movement{ RE::VTABLE_MovementHandler[0] };
    _ProcessThumbstick = movement.write_vfunc(0x2 + shift, ProcessThumbstick);
    _ProcessButton = movement.write_vfunc(0x4 + shift, ProcessButton);

    REL::Relocation<std::uintptr_t> look{ RE::VTABLE_LookHandler[0] };
    _LookThumbstick = look.write_vfunc(0x2 + shift, LookThumbstick);

    REL::Relocation<std::uintptr_t> third{ RE::VTABLE_ThirdPersonState[0] };
    _SetFreeRotationMode = third.write_vfunc(0xD, SetFreeRotationMode);

    // CanProcess's slot on 1.7.99+ is not known; there the gamepad buttons
    // the core uses still reach Skyrim's shout and tween menu.
    if (shift == 0) {
        REL::Relocation<std::uintptr_t> shout{ RE::VTABLE_ShoutHandler[0] };
        _ShoutCanProcess = shout.write_vfunc(0x1, ShoutCanProcess);

        REL::Relocation<std::uintptr_t> menuOpen{ RE::VTABLE_MenuOpenHandler[0] };
        _MenuOpenCanProcess = menuOpen.write_vfunc(0x1, MenuOpenCanProcess);
    }

    logger::info("Hooked movement, look, camera rotation, shout and menu-open input");
}

void Movement::SetActive(bool on)
{
    if (active == on) {
        return;
    }
    active = on;
    move = RE::NiPoint2(0.0f, 0.0f);
    lookX = lookY = 0.0f;
    if (auto* third = ThirdPerson()) {
        if (on) {
            third->freeRotationEnabled = true;
        }
    }
    logger::info("Elden Ring movement and camera {}", on ? "on" : "off");
}

void Movement::SetMove(float x, float y)
{
    move = RE::NiPoint2(x, y);
    if (auto* controls = RE::PlayerControls::GetSingleton(); controls && active) {
        controls->data.moveInputVec = move;
    }
}

RE::ThirdPersonState* Movement::ThirdPerson()
{
    auto* camera = RE::PlayerCamera::GetSingleton();
    if (!camera || !camera->currentState || camera->currentState->id != RE::CameraState::kThirdPerson) {
        return nullptr;
    }
    return static_cast<RE::ThirdPersonState*>(camera->currentState.get());
}

float Movement::CameraHeading(RE::PlayerCharacter* player)
{
    float heading = player->GetAngleZ();
    if (auto* third = ThirdPerson(); third && third->freeRotationEnabled) {
        heading += third->freeRotation.x;
    }
    return Wrap(heading);
}

void Movement::Turn(RE::PlayerCharacter* player, float heading)
{
    const float delta = Wrap(heading - player->GetAngleZ());
    if (std::abs(delta) < 1e-5f) {
        return;
    }
    player->SetRotationZ(Wrap(heading));
    // The camera stays put while the player turns under it.
    if (auto* third = ThirdPerson(); active && third && third->freeRotationEnabled) {
        third->freeRotation.x = Wrap(third->freeRotation.x - delta);
    }
}

void Movement::SteerCamera(RE::PlayerCharacter* player, float yaw, float delta)
{
    auto* third = ThirdPerson();
    if (!active || !third || !third->freeRotationEnabled) {
        return;
    }
    const float want = Wrap(yaw - player->GetAngleZ());
    const float diff = Wrap(want - third->freeRotation.x);
    const float t = std::min(1.0f, std::max(0.0f, delta) * Settings::Get().lockCameraSpeed);
    third->freeRotation.x = Wrap(third->freeRotation.x + diff * t);
}

void Movement::ProcessThumbstick(RE::MovementHandler* a_this, RE::ThumbstickEvent* a_event, RE::PlayerControlsData* a_data)
{
    _ProcessThumbstick(a_this, a_event, a_data);
    if (active && a_data) {
        a_data->moveInputVec = move;
    }
}

void Movement::ProcessButton(RE::MovementHandler* a_this, RE::ButtonEvent* a_event, RE::PlayerControlsData* a_data)
{
    _ProcessButton(a_this, a_event, a_data);
    if (active && a_data) {
        a_data->moveInputVec = move;
    }
}

void Movement::LookThumbstick(RE::LookHandler* a_this, RE::ThumbstickEvent* a_event, RE::PlayerControlsData* a_data)
{
    const float smoothing = Settings::Get().padLookSmoothing;
    if (!active || !a_event || !a_event->IsRight() || smoothing <= 0.0f) {
        _LookThumbstick(a_this, a_event, a_data);
        return;
    }
    // Elden Ring's camera eases into a turn instead of jumping to full speed;
    // letting go stops it at once.
    const auto now = std::chrono::steady_clock::now();
    const float dt = std::clamp(std::chrono::duration<float>(now - lastLook).count(), 0.0f, 0.1f);
    lastLook = now;
    const float rawX = a_event->xValue, rawY = a_event->yValue;
    if (rawX * rawX + rawY * rawY < 1e-4f) {
        lookX = lookY = 0.0f;
    } else {
        const float t = std::min(1.0f, dt / smoothing);
        lookX += (rawX - lookX) * t;
        lookY += (rawY - lookY) * t;
    }
    a_event->xValue = lookX;
    a_event->yValue = lookY;
    _LookThumbstick(a_this, a_event, a_data);
    a_event->xValue = rawX;
    a_event->yValue = rawY;
}

void Movement::SetFreeRotationMode(RE::ThirdPersonState* a_this, bool a_weaponSheathed)
{
    if (!active) {
        _SetFreeRotationMode(a_this, a_weaponSheathed);
        return;
    }
    // Keep the camera free with the weapon drawn (Skyrim would turn the
    // player with it). As TDM does, the camera's pitch moves onto the
    // player's own pitch, where Skyrim expects it with a weapon out.
    a_this->freeRotationEnabled = true;
    auto* player = RE::PlayerCharacter::GetSingleton();
    auto* camera = static_cast<RE::PlayerCamera*>(a_this->camera);
    if (player && camera && camera->cameraTarget.get().get() == player) {
        const float pitch = -a_this->freeRotation.y;
        player->data.angle.x += pitch;
        a_this->freeRotation.y += pitch;
    }
}

bool Movement::ShoutCanProcess(RE::ShoutHandler* a_this, RE::InputEvent* a_event)
{
    if (active && a_event && Input::Get()->OwnsPadButton(a_event)) {
        return false;
    }
    return _ShoutCanProcess(a_this, a_event);
}

bool Movement::MenuOpenCanProcess(RE::MenuOpenHandler* a_this, RE::InputEvent* a_event)
{
    // B rolls in Elden Ring mode; it must not open the tween menu as well.
    if (active && a_event && Input::Get()->OwnsPadButton(a_event)) {
        return false;
    }
    return _MenuOpenCanProcess(a_this, a_event);
}
