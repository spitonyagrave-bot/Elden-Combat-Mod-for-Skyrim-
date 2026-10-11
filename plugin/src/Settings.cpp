#include "Settings.h"

#include <Windows.h>

namespace
{
    constexpr auto kPath = L"Data/SKSE/Plugins/ERCombat.ini";

    std::uint32_t ReadKey(const wchar_t* key, std::uint32_t fallback)
    {
        return static_cast<std::uint32_t>(GetPrivateProfileIntW(L"Keys", key, static_cast<int>(fallback), kPath));
    }

    std::string ReadEvent(const wchar_t* key, const char* fallback)
    {
        wchar_t buffer[128]{};
        std::wstring wideFallback(fallback, fallback + std::strlen(fallback));
        GetPrivateProfileStringW(L"Events", key, wideFallback.c_str(), buffer, 128, kPath);
        std::string out;
        for (const wchar_t* c = buffer; *c; ++c) {
            out.push_back(static_cast<char>(*c));
        }
        return out;
    }
}

Settings& Settings::Get()
{
    static Settings settings;
    return settings;
}

void Settings::Load()
{
    toggleKey = ReadKey(L"Toggle", toggleKey);
    dodgeKey = ReadKey(L"Dodge", dodgeKey);
    jumpKey = ReadKey(L"Jump", jumpKey);
    lightKey = ReadKey(L"Light", lightKey);
    guardKey = ReadKey(L"Guard", guardKey);
    heavyModifier = ReadKey(L"HeavyModifier", heavyModifier);
    walkKey = ReadKey(L"Walk", walkKey);

    // Defaults drive the animations Gate to Sovngarde already has: TK Dodge
    // for rolls, and the vanilla attack events that MCO/ADXP and BFCO answer.
    events[ER_ACTION_ROLL] = ReadEvent(L"Roll", "TKDodge");
    events[ER_ACTION_CROUCH_ROLL] = ReadEvent(L"CrouchRoll", "TKDodge");
    events[ER_ACTION_BACKSTEP] = ReadEvent(L"Backstep", "TKDodgeBack");
    events[ER_ACTION_JUMP] = ReadEvent(L"Jump", "JumpStandingStart");
    events[ER_ACTION_LIGHT_ATTACK] = ReadEvent(L"LightAttack", "attackStart");
    // Skyrim has no held power attack, so the swing plays when the core
    // releases the charge.
    events[ER_ACTION_HEAVY_CHARGE] = ReadEvent(L"HeavyCharge", "");
    events[ER_ACTION_HEAVY_ATTACK] = ReadEvent(L"HeavyAttack", "attackPowerStartInPlace");
    events[ER_ACTION_RUN_ATTACK] = ReadEvent(L"RunAttack", "attackStartSprint");
    events[ER_ACTION_ROLL_ATTACK] = ReadEvent(L"RollAttack", "attackStart");
    events[ER_ACTION_BACKSTEP_ATTACK] = ReadEvent(L"BackstepAttack", "attackStart");
    events[ER_ACTION_CROUCH_ATTACK] = ReadEvent(L"CrouchAttack", "attackStart");
    events[ER_ACTION_GUARD_COUNTER] = ReadEvent(L"GuardCounter", "attackPowerStartInPlace");
    events[ER_ACTION_LEFT_ATTACK] = ReadEvent(L"LeftAttack", "attackStartLeftHand");
    events[ER_ACTION_HURT] = ReadEvent(L"Hurt", "");

    coreHitDetection = GetPrivateProfileIntW(L"General", L"CoreHitDetection", 0, kPath) != 0;
}
