cmake_minimum_required(VERSION 3.22)

project(wyldtone_juce_plugin LANGUAGES C CXX)

if(NOT DEFINED WYLDTONE_PLUGIN_NAME OR WYLDTONE_PLUGIN_NAME STREQUAL "")
    message(FATAL_ERROR "WYLDTONE_PLUGIN_NAME muss angegeben werden")
endif()

# Debug information for WASM backtraces
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "" FORCE)
endif()

set(JUCE_DIR "" CACHE PATH "Path to JUCE")
set(PLUGIN_DIR "" CACHE PATH "Path to plugin source directory")
set(WYLDTONE_JUCE_MODULES "" CACHE STRING "Optional JUCE modules for this plugin")
set(WYLDTONE_PLUGIN_EXTRA_INCLUDES "" CACHE STRING "Extra include directories for plugin headers")
option(WYLDTONE_ENABLE_SIMD128 "Enable WebAssembly SIMD128 for this plugin" OFF)
set(WYLDTONE_JUCE_MODE "upstream" CACHE STRING "Only unmodified external JUCE checkout is supported")
set(WYLDTONE_OUTPUT_DIR "" CACHE PATH "WASM destination")

set_property(CACHE WYLDTONE_JUCE_MODE PROPERTY STRINGS upstream)

# --------------------------------------------------------------------------
# Validate configuration
# --------------------------------------------------------------------------

if(NOT JUCE_DIR)
    message(FATAL_ERROR "JUCE_DIR is required")
endif()

if(NOT PLUGIN_DIR)
    message(FATAL_ERROR "PLUGIN_DIR is required")
endif()

# --------------------------------------------------------------------------
# Normalize paths
# --------------------------------------------------------------------------

# Rust canonicalize() on Windows may produce:
# \\?\C:\path
# or, after slash conversion:
# //?/C:/path

string(REPLACE "\\" "/" PLUGIN_DIR "${PLUGIN_DIR}")
string(REPLACE "\\" "/" JUCE_DIR "${JUCE_DIR}")

if(PLUGIN_DIR MATCHES "^//\\?/")
    string(SUBSTRING "${PLUGIN_DIR}" 4 -1 PLUGIN_DIR)
endif()

if(JUCE_DIR MATCHES "^//\\?/")
    string(SUBSTRING "${JUCE_DIR}" 4 -1 JUCE_DIR)
endif()

message(STATUS "Plugin directory: ${PLUGIN_DIR}")
message(STATUS "JUCE directory: ${JUCE_DIR}")

# --------------------------------------------------------------------------
# Select JUCE processor backend
# --------------------------------------------------------------------------

# Both modes use the official headless processor module.  The full
# juce_audio_processors module compiles JUCE's desktop plugin-host detection,
# which does not have an Emscripten platform branch.  Keep the upstream JUCE
# checkout untouched; any additional headless compatibility is SDK-owned.
if(NOT WYLDTONE_JUCE_MODE STREQUAL "upstream")
    message(FATAL_ERROR "Unsupported JUCE mode: ${WYLDTONE_JUCE_MODE}")
endif()

if(NOT EXISTS "${JUCE_DIR}/modules/juce_audio_processors_headless/juce_audio_processors_headless.h")
    message(FATAL_ERROR
        "The selected JUCE checkout has no juce_audio_processors_headless module. "
        "Use the supported JUCE commit before trying the headless build.")
endif()
set(WYLD_JUCE_PROCESSOR_TARGET juce::juce_audio_processors_headless)

message(STATUS "Wyldtone JUCE mode: ${WYLDTONE_JUCE_MODE}")
message(STATUS "Wyldtone JUCE processor target: ${WYLD_JUCE_PROCESSOR_TARGET}")

# --------------------------------------------------------------------------
# JUCE configuration
# --------------------------------------------------------------------------

set(JUCE_MODULES_ONLY ON CACHE BOOL "" FORCE)

set(JUCE_USE_CURL OFF CACHE BOOL "" FORCE)
set(JUCE_WEB_BROWSER OFF CACHE BOOL "" FORCE)
set(JUCE_HEADLESS_PLUGIN_CLIENT ON CACHE BOOL "" FORCE)

if(WYLDTONE_JUCE_MODE STREQUAL "upstream")
    add_compile_options("-include" "emscripten.h")
endif()

add_subdirectory(
    "${JUCE_DIR}"
    "${CMAKE_BINARY_DIR}/JUCE"
)

# --------------------------------------------------------------------------
# Plugin sources
# --------------------------------------------------------------------------

set(PLUGIN_ENTRY
    "${WYLDTONE_OUTPUT_DIR}/PluginEntry.cpp"
)

file(GLOB_RECURSE PLUGIN_SOURCES CONFIGURE_DEPENDS
    "${PLUGIN_DIR}/*.cpp"
    "${PLUGIN_DIR}/*.cc"
    "${PLUGIN_DIR}/*.cxx"
    "${PLUGIN_DIR}/*.c"
)

# Never compile graphical editors into a Wyldtone WASM module.
list(FILTER PLUGIN_SOURCES EXCLUDE REGEX "(^|/)PluginEditor\\.cpp$")
# Headless plugins do not compile graphical implementation sources.
list(FILTER PLUGIN_SOURCES EXCLUDE REGEX "(^|/)ui/.*\\.(cpp|cc|cxx|c)$")

message(STATUS "Plugin sources: ${PLUGIN_SOURCES}")

if(NOT PLUGIN_SOURCES)
    # A header-only AudioProcessor is instantiated in the SDK-generated
    # PluginEntry.cpp. It is the only plugin translation unit in this case.
    if(NOT EXISTS "${PLUGIN_ENTRY}")
        message(FATAL_ERROR "Missing generated PluginEntry.cpp for header-only JUCE plugin")
    endif()
    message(STATUS "Header-only JUCE plugin: compiling generated PluginEntry.cpp")
endif()

# --------------------------------------------------------------------------
# Plugin library
# --------------------------------------------------------------------------

add_library(wyldtone_plugin STATIC
    ${PLUGIN_SOURCES}
    "${PLUGIN_ENTRY}"
)

target_compile_features(wyldtone_plugin PRIVATE cxx_std_17)

foreach(_wyldtone_include IN LISTS WYLDTONE_PLUGIN_EXTRA_INCLUDES)
    if(NOT IS_DIRECTORY "${_wyldtone_include}")
        message(FATAL_ERROR "Missing plugin include directory: ${_wyldtone_include}")
    endif()
    target_include_directories(wyldtone_plugin PRIVATE "${_wyldtone_include}")
endforeach()

if(WYLDTONE_ENABLE_SIMD128)
    target_compile_options(wyldtone_plugin PRIVATE "-msimd128")
endif()

# --------------------------------------------------------------------------
# WASM compatibility for the unchanged upstream JUCE checkout
# --------------------------------------------------------------------------
# On this JUCE revision ThreadPriorities_native.h has an empty priority table
# for JUCE_WASM.  Clang's VFS overlay redirects just that header to a build-
# generated version. JUCE_DIR and the user's local JUCE checkout are NOT edited.
# No threading is enabled; as on JUCE's Linux/BSD mapping all native priority
# values in this mapping are zero. When JUCE supports WASM upstream, remove
# this overlay after verifying that the five-entry table is provided there.
if(WYLDTONE_JUCE_MODE STREQUAL "upstream")
    set(_wyldtone_priority_source
        "${JUCE_DIR}/modules/juce_core/native/juce_ThreadPriorities_native.h")
    if(NOT EXISTS "${_wyldtone_priority_source}")
        message(FATAL_ERROR "Cannot locate JUCE thread priority header: ${_wyldtone_priority_source}")
    endif()

    file(READ "${_wyldtone_priority_source}" _wyldtone_priority_text)
    set(_wyldtone_needle "#if JUCE_LINUX || JUCE_BSD")
    string(REGEX MATCHALL "#if[ \t]+JUCE_LINUX[ \t]*\\|\\|[ \t]*JUCE_BSD"
        _wyldtone_priority_matches "${_wyldtone_priority_text}")
    list(LENGTH _wyldtone_priority_matches _wyldtone_match_count)
    if(NOT _wyldtone_match_count EQUAL 1)
        message(FATAL_ERROR
            "JUCE ThreadPriorities layout changed: expected exactly one Linux/BSD "
            "priority-table condition, found ${_wyldtone_match_count}. "
            "Review this JUCE revision rather than silently patching it.")
    endif()
    string(REPLACE "${_wyldtone_needle}"
        "#if JUCE_LINUX || JUCE_BSD || JUCE_WASM"
        _wyldtone_compatible_priority_text "${_wyldtone_priority_text}")
    if(_wyldtone_priority_text STREQUAL _wyldtone_compatible_priority_text)
        message(FATAL_ERROR "JUCE WASM priority-table compatibility replacement did not apply")
    endif()

    set(_wyldtone_compat_dir "${CMAKE_CURRENT_BINARY_DIR}/wyldtone-juce-compat")
    file(MAKE_DIRECTORY "${_wyldtone_compat_dir}")
    set(_wyldtone_compat_header
        "${_wyldtone_compat_dir}/juce_ThreadPriorities_native.h")
    file(WRITE "${_wyldtone_compat_header}" "${_wyldtone_compatible_priority_text}")

    # Clang VFS overlays resolve source headers by their *original* full path.
    # Only this single file is redirected during compilation; no filesystem
    # writes occur under JUCE_DIR.
    # A second unsupported WASM branch occurs in juce_data_structures:
    # PropertiesFile::Options::getDefaultFile() never creates `dir` for JUCE_WASM.
    # Remap this compilation unit in the temporary VFS, just like the priority
    # header. The developer's JUCE checkout remains untouched.
    set(_wyldtone_properties_source
        "${JUCE_DIR}/modules/juce_data_structures/app_properties/juce_PropertiesFile.cpp")
    if(NOT EXISTS "${_wyldtone_properties_source}")
        message(FATAL_ERROR "Missing JUCE PropertiesFile source: ${_wyldtone_properties_source}")
    endif()
    file(READ "${_wyldtone_properties_source}" _wyldtone_properties_text)
    string(REPLACE "\r\n" "\n" _wyldtone_properties_text "${_wyldtone_properties_text}")

    # This is deliberately specific to the tested JUCE revision. Fail visibly
    # rather than silently rewriting unrelated code in a different release.
    set(_wyldtone_properties_anchor
        "   #elif JUCE_WINDOWS\n    auto dir = File::getSpecialLocation (commonToAllUsers")
    string(FIND "${_wyldtone_properties_text}" "${_wyldtone_properties_anchor}"
        _wyldtone_properties_anchor_position)
    if(_wyldtone_properties_anchor_position EQUAL -1)
        message(FATAL_ERROR
            "JUCE PropertiesFile layout changed: cannot locate the expected Windows "
            "getDefaultFile() branch. Review compatibility with this JUCE revision.")
    endif()
    string(CONCAT _wyldtone_properties_replacement
        "   #elif JUCE_WASM\n"
        "    // Compiled WASM has no host filesystem unless the runtime provides one.\n"
        "    // This path is only a default; Wyldtone plugins must not rely on it.\n"
        "    auto dir = File (\"/tmp\").getChildFile (\n"
        "        folderName.isNotEmpty() ? folderName : applicationName);\n"
        "\n"
        "   #elif JUCE_WINDOWS\n"
        "    auto dir = File::getSpecialLocation (commonToAllUsers")
    string(REPLACE "${_wyldtone_properties_anchor}"
        "${_wyldtone_properties_replacement}"
        _wyldtone_compatible_properties_text "${_wyldtone_properties_text}")
    if(_wyldtone_properties_text STREQUAL _wyldtone_compatible_properties_text)
        message(FATAL_ERROR "JUCE PropertiesFile WASM compatibility replacement did not apply")
    endif()
    set(_wyldtone_compat_properties
        "${_wyldtone_compat_dir}/juce_PropertiesFile.cpp")
    file(WRITE "${_wyldtone_compat_properties}"
        "${_wyldtone_compatible_properties_text}")

    # APVTS is part of juce_audio_processors (GUI module) rather than the
    # official headless implementation.  KFuzz depends on APVTS for its
    # parameters and state. Keep the host GUI out of WASM by selectively
    # disabling only the three GUI attachment classes at compile time.
    # Mirrors the narrow existing JUCE WASM compatibility patch, but leaves
    # the checked-out JUCE sources untouched.
    set(_wyldtone_apvts_header_source
        "${JUCE_DIR}/modules/juce_audio_processors/utilities/juce_AudioProcessorValueTreeState.h")
    set(_wyldtone_apvts_impl_source
        "${JUCE_DIR}/modules/juce_audio_processors/utilities/juce_AudioProcessorValueTreeState.cpp")
    foreach(_wyldtone_source IN ITEMS
            "${_wyldtone_apvts_header_source}"
            "${_wyldtone_apvts_impl_source}")
        if(NOT EXISTS "${_wyldtone_source}")
            message(FATAL_ERROR "Missing JUCE APVTS source: ${_wyldtone_source}")
        endif()
    endforeach()

    file(READ "${_wyldtone_apvts_header_source}" _wyldtone_apvts_header_text)
    file(READ "${_wyldtone_apvts_impl_source}" _wyldtone_apvts_impl_text)
    string(REPLACE "\r\n" "\n" _wyldtone_apvts_header_text "${_wyldtone_apvts_header_text}")
    string(REPLACE "\r\n" "\n" _wyldtone_apvts_impl_text "${_wyldtone_apvts_impl_text}")

    set(_wyldtone_apvts_header_start
        "    /** An object of this class maintains a connection between a Slider and a parameter")
    set(_wyldtone_apvts_header_end
        "JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (ButtonAttachment)\n    };")
    string(FIND "${_wyldtone_apvts_header_text}" "${_wyldtone_apvts_header_start}"
        _wyldtone_header_start_pos)
    string(FIND "${_wyldtone_apvts_header_text}" "${_wyldtone_apvts_header_end}"
        _wyldtone_header_end_pos)
    if(_wyldtone_header_start_pos EQUAL -1 OR _wyldtone_header_end_pos EQUAL -1)
        message(FATAL_ERROR
            "Unsupported JUCE revision: APVTS GUI attachment header anchors missing")
    endif()
    string(REPLACE "${_wyldtone_apvts_header_start}"
        "   #if ! JUCE_WASM\n${_wyldtone_apvts_header_start}"
        _wyldtone_apvts_header_compat "${_wyldtone_apvts_header_text}")
    string(REPLACE "${_wyldtone_apvts_header_end}"
        "${_wyldtone_apvts_header_end}\n   #endif // ! JUCE_WASM"
        _wyldtone_apvts_header_compat "${_wyldtone_apvts_header_compat}")

    set(_wyldtone_apvts_impl_start
        "AudioProcessorValueTreeState::SliderAttachment::SliderAttachment")
    set(_wyldtone_apvts_impl_end
        "    : attachment (makeAttachment<ButtonParameterAttachment> (stateToUse, parameterID, button))\n{\n}")
    string(FIND "${_wyldtone_apvts_impl_text}" "${_wyldtone_apvts_impl_start}"
        _wyldtone_impl_start_pos)
    string(FIND "${_wyldtone_apvts_impl_text}" "${_wyldtone_apvts_impl_end}"
        _wyldtone_impl_end_pos)
    if(_wyldtone_impl_start_pos EQUAL -1 OR _wyldtone_impl_end_pos EQUAL -1)
        message(FATAL_ERROR
            "Unsupported JUCE revision: APVTS GUI attachment implementation anchors missing")
    endif()
    string(REPLACE "${_wyldtone_apvts_impl_start}"
        "#if ! JUCE_WASM\n${_wyldtone_apvts_impl_start}"
        _wyldtone_apvts_impl_compat "${_wyldtone_apvts_impl_text}")
    string(REPLACE "${_wyldtone_apvts_impl_end}"
        "${_wyldtone_apvts_impl_end}\n#endif // ! JUCE_WASM"
        _wyldtone_apvts_impl_compat "${_wyldtone_apvts_impl_compat}")

    set(_wyldtone_apvts_header_compat_path
        "${_wyldtone_compat_dir}/juce_AudioProcessorValueTreeState.h")
    set(_wyldtone_apvts_impl_compat_path
        "${_wyldtone_compat_dir}/juce_AudioProcessorValueTreeState.cpp")
    file(WRITE "${_wyldtone_apvts_header_compat_path}"
        "${_wyldtone_apvts_header_compat}")
    file(WRITE "${_wyldtone_apvts_impl_compat_path}"
        "${_wyldtone_apvts_impl_compat}")

    # The VFS remaps four files while JUCE_DIR stays completely unchanged.
    set(_wyldtone_overlay "${_wyldtone_compat_dir}/juce-wasm-overlay.yaml")
    file(TO_CMAKE_PATH "${JUCE_DIR}/modules/juce_core/native" _wyldtone_original_dir)
    file(TO_CMAKE_PATH "${_wyldtone_compat_header}" _wyldtone_compat_header_vfs)
    file(TO_CMAKE_PATH "${JUCE_DIR}/modules/juce_data_structures/app_properties" _wyldtone_properties_dir_vfs)
    file(TO_CMAKE_PATH "${_wyldtone_compat_properties}" _wyldtone_compat_properties_vfs)
    file(TO_CMAKE_PATH "${JUCE_DIR}/modules/juce_audio_processors/utilities" _wyldtone_apvts_dir_vfs)
    file(TO_CMAKE_PATH "${_wyldtone_apvts_header_compat_path}" _wyldtone_apvts_header_vfs)
    file(TO_CMAKE_PATH "${_wyldtone_apvts_impl_compat_path}" _wyldtone_apvts_impl_vfs)
    file(WRITE "${_wyldtone_overlay}"
        "{\n  \"version\": 0,\n  \"case-sensitive\": \"false\",\n  \"roots\": [\n"
        "    {\n      \"type\": \"directory\",\n      \"name\": \"${_wyldtone_original_dir}\",\n"
        "      \"contents\": [{\"type\": \"file\", \"name\": \"juce_ThreadPriorities_native.h\", \"external-contents\": \"${_wyldtone_compat_header_vfs}\"}]\n    },\n"
        "    {\n      \"type\": \"directory\",\n      \"name\": \"${_wyldtone_properties_dir_vfs}\",\n"
        "      \"contents\": [{\"type\": \"file\", \"name\": \"juce_PropertiesFile.cpp\", \"external-contents\": \"${_wyldtone_compat_properties_vfs}\"}]\n    },\n"
        "    {\n      \"type\": \"directory\",\n      \"name\": \"${_wyldtone_apvts_dir_vfs}\",\n"
        "      \"contents\": [\n"
        "        {\"type\": \"file\", \"name\": \"juce_AudioProcessorValueTreeState.h\", \"external-contents\": \"${_wyldtone_apvts_header_vfs}\"},\n"
        "        {\"type\": \"file\", \"name\": \"juce_AudioProcessorValueTreeState.cpp\", \"external-contents\": \"${_wyldtone_apvts_impl_vfs}\"}\n"
        "      ]\n    }\n  ]\n}\n")
    target_compile_options(wyldtone_plugin PRIVATE
        "SHELL:-ivfsoverlay \"${_wyldtone_overlay}\"")
    message(STATUS "JUCE WASM compatibility VFS overlay (priority + PropertiesFile): ${_wyldtone_overlay}")
endif()

target_compile_definitions(wyldtone_plugin PRIVATE
    "JucePlugin_Name=\"${WYLDTONE_PLUGIN_NAME}\""
)


target_include_directories(wyldtone_plugin
    PRIVATE
        "${WYLDTONE_OUTPUT_DIR}/generated"
        "${PLUGIN_DIR}"
        "${PLUGIN_DIR}/src"
        "${CMAKE_CURRENT_SOURCE_DIR}/../bridge"
)

# --------------------------------------------------------------------------
# Optional JUCE modules
# --------------------------------------------------------------------------

set(WYLD_JUCE_TARGETS "")

foreach(module IN LISTS WYLDTONE_JUCE_MODULES)
    if(NOT TARGET juce::${module})
        message(FATAL_ERROR
            "Requested JUCE module is unavailable: ${module}"
        )
    endif()

    list(APPEND WYLD_JUCE_TARGETS juce::${module})
endforeach()

# --------------------------------------------------------------------------
# Link plugin library
# --------------------------------------------------------------------------

target_link_libraries(wyldtone_plugin
    PRIVATE
        ${WYLD_JUCE_PROCESSOR_TARGET}
        ${WYLD_JUCE_TARGETS}
)

# --------------------------------------------------------------------------
# Final WASM module
# --------------------------------------------------------------------------

add_executable(wyldtone_plugin_wasm
    "${CMAKE_CURRENT_SOURCE_DIR}/../bridge/WasmExport.cpp"
)

# The headless JUCE target intentionally does not compile APVTS, provide
# a desktop message queue, or link juce_graphics. Supply the narrowly scoped
# SDK compatibility implementations while keeping JUCE itself unmodified.
if(WYLDTONE_JUCE_MODE STREQUAL "upstream")
    if("juce_graphics" IN_LIST WYLDTONE_JUCE_MODULES)
        message(FATAL_ERROR
            "Headless ColourCompat cannot be linked with juce_graphics. "
            "Remove juce_graphics from the headless module list or use a GUI-capable backend.")
    endif()
    target_sources(wyldtone_plugin_wasm PRIVATE
        "${CMAKE_CURRENT_SOURCE_DIR}/../bridge/HeadlessAPVTS.cpp"
        "${CMAKE_CURRENT_SOURCE_DIR}/../bridge/WasmPlatformCompat.cpp"
        "${CMAKE_CURRENT_SOURCE_DIR}/../headless/ColourCompat.cpp"
    )
endif()


set_target_properties(wyldtone_plugin_wasm PROPERTIES
    OUTPUT_NAME "plugin"
    SUFFIX ".wasm"
    RUNTIME_OUTPUT_DIRECTORY "${WYLDTONE_OUTPUT_DIR}"
)

target_compile_features(wyldtone_plugin_wasm PRIVATE cxx_std_17)

if(WYLDTONE_ENABLE_SIMD128)
    target_compile_options(wyldtone_plugin_wasm PRIVATE "-msimd128")
    target_link_options(wyldtone_plugin_wasm PRIVATE "-msimd128")
endif()

# JUCE's interface modules contribute compilation units to BOTH targets:
# wyldtone_plugin (static library) and wyldtone_plugin_wasm (final executable).
# The VFS overlay must therefore be applied to both compilation contexts.
# Otherwise juce_core.cpp is compiled again without the WASM compatibility
# headers and fails in juce_ThreadPriorities_native.h.
if(WYLDTONE_JUCE_MODE STREQUAL "upstream")
    target_compile_options(wyldtone_plugin_wasm PRIVATE
        "SHELL:-ivfsoverlay \"${_wyldtone_overlay}\"")
endif()

target_link_libraries(wyldtone_plugin_wasm
    PRIVATE
        wyldtone_plugin
        ${WYLD_JUCE_PROCESSOR_TARGET}
        ${WYLD_JUCE_TARGETS}
)

# --------------------------------------------------------------------------
# WASM linker options
# --------------------------------------------------------------------------

target_link_options(wyldtone_plugin_wasm
    PRIVATE
        "-sSTANDALONE_WASM=1"
        "-sEXPORTED_FUNCTIONS=['_plugin_create', '_plugin_init', '_plugin_reset', '_plugin_pending_events', '_plugin_input_bus_count', '_plugin_output_bus_count', '_plugin_input_bus_channels', '_plugin_output_bus_channels', '_plugin_state_version', '_plugin_state_capture', '_plugin_state_buffer_ptr', '_plugin_state_prepare_load', '_plugin_state_commit_load', '_plugin_name', '_plugin_buffer_ptr', '_plugin_process', '_plugin_process_buses', '_plugin_parameter_count', '_plugin_parameter_id', '_plugin_parameter_name', '_plugin_parameter_label', '_plugin_parameter_min', '_plugin_parameter_max', '_plugin_parameter_default', '_plugin_parameter_step', '_plugin_parameter_flags', '_plugin_parameter_choice', '_plugin_parameter_get', '_plugin_parameter_set']"
        "-sALLOW_MEMORY_GROWTH=0"
        "--no-entry"
)

