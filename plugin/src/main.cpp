#include "Bridge.h"
#include "Input.h"
#include "Integrations.h"
#include "PoseDriver.h"
#include "Settings.h"
#include "Win32.h"

namespace
{
    void SetupLog()
    {
        auto dir = Win32::LogDirectory();
        if (!dir) {
            return;
        }
        auto path = *dir / "ERCombat.log";
        auto sink = std::make_shared<spdlog::sinks::basic_file_sink_mt>(path.string(), true);
        auto log = std::make_shared<spdlog::logger>("global", std::move(sink));
        log->set_level(spdlog::level::info);
        log->flush_on(spdlog::level::info);
        spdlog::set_default_logger(std::move(log));
    }

    const char* LoadResultText(std::int32_t result)
    {
        switch (result) {
        case ER_LOAD_BAD_PATH:
            return "the path isn't valid UTF-8";
        case ER_LOAD_IO:
            return "missing or unreadable";
        case ER_LOAD_NOT_ERDATA:
            return "not an .erdata file";
        case ER_LOAD_VERSION:
            return "written for another version of the core";
        case ER_LOAD_LAYOUT:
            return "written by a core with different action lists";
        case ER_LOAD_CORRUPT:
            return "cut short or damaged";
        case ER_LOAD_ALREADY_LOADED:
            return "a table was already loaded";
        default:
            return "unknown error";
        }
    }

    // The Elden Ring action table, generated from your own copy of the game
    // and dumped with er_dump. Without it the core keeps its stand-in table:
    // everything works, but timings and moves are placeholders.
    void LoadData()
    {
        auto dir = Win32::PluginDirectory();
        if (!dir) {
            logger::warn("Couldn't find the plugin folder for ERCombat.erdata; using the stand-in action table");
            return;
        }
        auto u8path = (*dir / "ERCombat.erdata").u8string();
        std::string path(reinterpret_cast<const char*>(u8path.c_str()), u8path.size());
        if (auto result = er_load_data(path.c_str()); result != ER_LOAD_OK) {
            logger::warn("Couldn't load {} ({}, code {}); using the stand-in action table", path, LoadResultText(result), result);
            return;
        }
        logger::info("Loaded the Elden Ring action table from {}", path);
    }

    void OnMessage(SKSE::MessagingInterface::Message* message)
    {
        if (message->type == SKSE::MessagingInterface::kDataLoaded) {
            Input::Get()->Register();
            logger::info("Listening to input");
            Integrations::Load();
        }
    }
}

SKSEPluginLoad(const SKSE::LoadInterface* skse)
{
    SKSE::Init(skse);
    SetupLog();
    logger::info("ERCombat {} loading", SKSE::PluginDeclaration::GetSingleton()->GetVersion().string());

    if (auto error = Win32::LoadCore(); error != 0) {
        logger::error("Couldn't load er_combat_core.dll next to ERCombat.dll (Windows error {}); combat stays off", error);
        return true;
    }
    LoadData();

    Settings::Get().Load();
    Bridge::Install();
    PoseDriver::Install();
    SKSE::GetMessagingInterface()->RegisterListener(OnMessage);
    return true;
}
