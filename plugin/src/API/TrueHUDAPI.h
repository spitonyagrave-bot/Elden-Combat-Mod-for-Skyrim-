#pragma once
// The part of TrueHUD's modder API (by ersh1, https://github.com/ersh1/TrueHUD,
// src/TrueHUDAPI.h, interface V1) this plugin uses: putting the lock-on
// target marker on our target. The interface is a vtable, so every function
// before the last one used is declared in TrueHUD's order; the ones not used
// are declared with placeholder parameters (never called).

namespace TrueHUD_API
{
    enum class InterfaceVersion : std::uint8_t
    {
        V1,
        V2,
        V3,
        V4
    };

    enum class APIResult : std::uint8_t
    {
        OK,
        NotOwner,
        MustKeep,
        AlreadyGiven,
        AlreadyTaken,
        BadThread,
    };

    class IVTrueHUD1
    {
    public:
        [[nodiscard]] virtual unsigned long GetTrueHUDThreadId() const noexcept = 0;
        [[nodiscard]] virtual APIResult RequestTargetControl(SKSE::PluginHandle a_myPluginHandle) noexcept = 0;
        [[nodiscard]] virtual APIResult RequestSpecialResourceBarsControl(SKSE::PluginHandle a_myPluginHandle) noexcept = 0;
        virtual APIResult SetTarget(SKSE::PluginHandle a_myPluginHandle, RE::ActorHandle a_actorHandle) noexcept = 0;
        virtual APIResult SetSoftTarget(SKSE::PluginHandle a_myPluginHandle, RE::ActorHandle a_actorHandle) noexcept = 0;
        virtual void Unused05() noexcept = 0;  // AddActorInfoBar
        virtual void Unused06() noexcept = 0;  // RemoveActorInfoBar
        virtual void Unused07() noexcept = 0;  // AddBoss
        virtual void Unused08() noexcept = 0;  // RemoveBoss
        virtual void Unused09() noexcept = 0;  // FlashActorValue
        virtual void Unused10() noexcept = 0;  // FlashActorSpecialBar
        virtual void Unused11() noexcept = 0;  // RegisterSpecialResourceFunctions
        virtual void Unused12() noexcept = 0;  // LoadCustomWidgets
        virtual void Unused13() noexcept = 0;  // RegisterNewWidgetType
        virtual void Unused14() noexcept = 0;  // AddWidget
        virtual void Unused15() noexcept = 0;  // RemoveWidget
        virtual SKSE::PluginHandle GetTargetControlOwner() const noexcept = 0;
        virtual SKSE::PluginHandle Unused17() const noexcept = 0;  // GetPlayerWidgetBarColorsControlOwner
        virtual SKSE::PluginHandle Unused18() const noexcept = 0;  // GetSpecialResourceBarControlOwner
        virtual APIResult ReleaseTargetControl(SKSE::PluginHandle a_myPluginHandle) noexcept = 0;
    };
}
