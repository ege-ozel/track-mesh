#pragma once
#include "bridge.h"
#include "public.sdk/source/common/pluginview.h"

namespace TrackMesh {
class View final : public Steinberg::CPluginView {
public:
    explicit View(std::shared_ptr<Bridge> bridge);
    ~View() override;
    Steinberg::tresult PLUGIN_API isPlatformTypeSupported(Steinberg::FIDString type) override;
    Steinberg::tresult PLUGIN_API attached(void* parent, Steinberg::FIDString type) override;
    Steinberg::tresult PLUGIN_API removed() override;
private:
    static LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam, LPARAM lparam);
    void paint(HWND window);
    std::shared_ptr<Bridge> bridge;
    HWND window {nullptr}, sendButton {nullptr}, receiveButton {nullptr}, uuid {nullptr};
    HFONT font {nullptr}, titleFont {nullptr};
    HBRUSH backgroundBrush {nullptr};
    HMODULE viewModule {nullptr};
    bool registeredClass {false};
};
}
