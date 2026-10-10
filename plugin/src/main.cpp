#include "Bridge.h"
#include "CoreLoader.h"
#include "Input.h"
#include "Settings.h"

namespace
{
    void SetupLog()
    {
        auto dir = logger::log_directory();
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

    void OnMessage(SKSE::MessagingInterface::Message* message)
    {
        if (message->type == SKSE::MessagingInterface::kDataLoaded) {
            Input::Get()->Register();
            logger::info("Listening to input");
        }
    }
}

SKSEPluginLoad(const SKSE::LoadInterface* skse)
{
    SKSE::Init(skse);
    SetupLog();
    logger::info("ERCombat {} loading", SKSE::PluginDeclaration::GetSingleton()->GetVersion().string());

    if (auto error = CoreLoader::Load(); error != 0) {
        logger::error("Couldn't load er_combat_core.dll next to ERCombat.dll (Windows error {}); combat stays off", error);
        return true;
    }

    Settings::Get().Load();
    Bridge::Install();
    SKSE::GetMessagingInterface()->RegisterListener(OnMessage);
    return true;
}
