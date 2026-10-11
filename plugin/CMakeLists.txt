cmake_minimum_required(VERSION 3.24)
project(ERCombat VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(CommonLibSSE CONFIG REQUIRED)

# The Rust core, built with `cargo build --release` in ../core.
set(ER_CORE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../core")
set(ER_CORE_LIB "${ER_CORE_DIR}/target/release/er_combat_core.dll.lib")
if(NOT EXISTS "${ER_CORE_LIB}")
    message(FATAL_ERROR "Build the core first: cd core && cargo build --release")
endif()

add_commonlibsse_plugin(${PROJECT_NAME}
    AUTHOR "spitonyagrave-bot"
    SOURCES
        src/main.cpp
        src/Bridge.cpp
        src/Input.cpp
        src/Integrations.cpp
        src/Weapons.cpp
        src/PoseDriver.cpp
        src/Settings.cpp
        src/Win32.cpp
)
target_precompile_headers(${PROJECT_NAME} PRIVATE src/PCH.h)
target_include_directories(${PROJECT_NAME} PRIVATE src "${ER_CORE_DIR}/include")
set_source_files_properties(src/Win32.cpp PROPERTIES SKIP_PRECOMPILE_HEADERS ON)
target_link_libraries(${PROJECT_NAME} PRIVATE "${ER_CORE_LIB}" delayimp shell32 ole32)
# SKSE loads plugins by full path, but Windows only looks for their
# dependencies next to SkyrimSE.exe. Delay-load the core and load it from
# the plugin folder ourselves (Win32::LoadCore) before anything calls into it.
target_link_options(${PROJECT_NAME} PRIVATE "/DELAYLOAD:er_combat_core.dll")

# Collect what goes into the mod archive.
add_custom_command(TARGET ${PROJECT_NAME} POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:${PROJECT_NAME}> "${CMAKE_CURRENT_SOURCE_DIR}/dist/SKSE/Plugins/"
    COMMAND ${CMAKE_COMMAND} -E copy "${ER_CORE_DIR}/target/release/er_combat_core.dll" "${CMAKE_CURRENT_SOURCE_DIR}/dist/SKSE/Plugins/"
)
