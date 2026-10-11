#include "Integrations.h"

#include "API/TrueDirectionalMovementAPI.h"
#include "API/TrueHUDAPI.h"
#include "Win32.h"

namespace
{
    TDM_API::IVTDM1* tdm = nullptr;
    TDM_API::IVTDM2* tdm2 = nullptr;  // yaw control
    TDM_API::IVTDM5* tdm5 = nullptr;  // target lock off
    TrueHUD_API::IVTrueHUD1* hud = nullptr;

    bool erMode = false;
    bool yawWanted = false;
    bool yawOwned = false;
    bool directionalWanted = false;
    bool directionalPaused = false;
    bool lockDisabled = false;
    bool hudTarget = false;
    RE::ActorHandle shownTarget;

    const char* Text(TDM_API::APIResult r)
    {
        switch (r) {
        case TDM_API::APIResult::OK:
            return "OK";
        case TDM_API::APIResult::NotOwner:
            return "not owner";
        case TDM_API::APIResult::MustKeep:
            return "TDM must keep it";
        case TDM_API::APIResult::AlreadyGiven:
            return "already ours";
        case TDM_API::APIResult::AlreadyTaken:
            return "taken by another mod";
        case TDM_API::APIResult::BadThread:
            return "wrong thread";
        default:
            return "?";
        }
    }

    bool Granted(TDM_API::APIResult r)
    {
        return r == TDM_API::APIResult::OK || r == TDM_API::APIResult::AlreadyGiven;
    }
}

namespace Integrations
{
    void Load()
    {
        // The newest interface TDM offers, down to the first.
        for (int v = static_cast<int>(TDM_API::InterfaceVersion::V5); v >= 0 && !tdm; --v) {
            auto* api = Win32::RequestPluginApi(L"TrueDirectionalMovement.dll", static_cast<std::uint8_t>(v));
            if (!api) {
                continue;
            }
            // Each version's interface extends the one before, so one object
            // serves them all.
            tdm = static_cast<TDM_API::IVTDM1*>(static_cast<TDM_API::IVTDM5*>(api));
            if (v >= static_cast<int>(TDM_API::InterfaceVersion::V2)) {
                tdm2 = static_cast<TDM_API::IVTDM2*>(static_cast<TDM_API::IVTDM5*>(api));
            }
            if (v >= static_cast<int>(TDM_API::InterfaceVersion::V5)) {
                tdm5 = static_cast<TDM_API::IVTDM5*>(api);
            }
            logger::info("True Directional Movement API V{} found", v + 1);
        }
        if (!tdm) {
            logger::info("True Directional Movement not found (or too old for its API): nothing to pause");
        } else if (!tdm5) {
            logger::warn("True Directional Movement is older than API V5: its target lock can't be switched off while Elden Ring mode is on. Don't press its lock key then.");
        }
        if (auto* api = Win32::RequestPluginApi(L"TrueHUD.dll", static_cast<std::uint8_t>(TrueHUD_API::InterfaceVersion::V1))) {
            hud = static_cast<TrueHUD_API::IVTrueHUD1*>(api);
            logger::info("TrueHUD API found: it shows our lock-on target");
        } else {
            logger::info("TrueHUD not found: no lock-on marker");
        }
    }

    void SetErMode(bool on)
    {
        if (on == erMode) {
            return;
        }
        erMode = on;
        const auto handle = SKSE::GetPluginHandle();
        if (on) {
            if (tdm5) {
                const auto r = tdm5->RequestDisableTargetLock(handle);
                lockDisabled = Granted(r);
                logger::info("TDM target lock off for Elden Ring mode: {}", Text(r));
            }
            if (hud) {
                const auto r = hud->RequestTargetControl(handle);
                hudTarget = r == TrueHUD_API::APIResult::OK || r == TrueHUD_API::APIResult::AlreadyGiven;
                logger::info("TrueHUD target marker control: {}", hudTarget ? "granted" : "refused");
            }
        } else {
            Update(false, 0.0f, false, nullptr);
            if (tdm5 && lockDisabled) {
                tdm5->ReleaseDisableTargetLock(handle);
                lockDisabled = false;
                logger::info("TDM target lock handed back");
            }
            if (hud && hudTarget) {
                hud->SetTarget(handle, RE::ActorHandle());
                hud->ReleaseTargetControl(handle);
                hudTarget = false;
            }
        }
    }

    void Update(bool ownYaw, float heading, bool pauseDirectional, RE::Actor* target)
    {
        const auto handle = SKSE::GetPluginHandle();
        if (tdm2) {
            if (ownYaw != yawWanted) {
                yawWanted = ownYaw;
                if (ownYaw) {
                    const auto r = tdm2->RequestYawControl(handle, 0.0f);
                    yawOwned = Granted(r);
                    if (!yawOwned) {
                        logger::warn("TDM refused yaw control: {}", Text(r));
                    }
                } else if (yawOwned) {
                    tdm2->ReleaseYawControl(handle);
                    yawOwned = false;
                }
            }
            if (yawOwned) {
                tdm2->SetPlayerYaw(handle, heading);
            }
        }
        if (tdm && pauseDirectional != directionalWanted) {
            directionalWanted = pauseDirectional;
            if (pauseDirectional) {
                const auto r = tdm->RequestDisableDirectionalMovement(handle);
                directionalPaused = Granted(r);
                logger::info("TDM directional movement paused while locked on: {}", Text(r));
            } else if (directionalPaused) {
                tdm->ReleaseDisableDirectionalMovement(handle);
                directionalPaused = false;
                logger::info("TDM directional movement back");
            }
        }
        if (hud && hudTarget) {
            RE::ActorHandle wanted = target ? target->GetHandle() : RE::ActorHandle();
            if (wanted.native_handle() != shownTarget.native_handle()) {
                shownTarget = wanted;
                hud->SetTarget(handle, wanted);
            }
        }
    }
}
