#pragma once
#include "bridge.h"
#include "public.sdk/source/common/pluginview.h"

namespace TrackMesh {
class Plugin;
class View final : public Steinberg::CPluginView {
public:
    View(std::shared_ptr<Bridge> bridge, Plugin* plugin);
    ~View() override;
    Steinberg::tresult PLUGIN_API isPlatformTypeSupported(Steinberg::FIDString type) override;
    Steinberg::tresult PLUGIN_API attached(void* parent, Steinberg::FIDString type) override;
    Steinberg::tresult PLUGIN_API removed() override;
private:
    static LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
    void paint(HWND window);
    void refresh();
    std::shared_ptr<Bridge> bridge;
    Plugin* plugin;
    std::vector<ProjectChoice> choices;
    bool changing {false};
    std::wstring selectionError;
    std::wstring drawnLabels; // language, Get label and share-on-save state the owner-drawn controls were painted with
    HWND window {nullptr}, sendButton {nullptr}, receiveButton {nullptr};
    HWND picker {nullptr}, chooseButton {nullptr}, changeButton {nullptr}, openButton {nullptr};
    HWND autoShareCheckbox {nullptr};
    HFONT font {nullptr}, titleFont {nullptr}, smallFont {nullptr};
    HBRUSH backgroundBrush {nullptr};
    HMODULE viewModule {nullptr};
    bool registeredClass {false};
};
}
