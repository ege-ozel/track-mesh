#include "view.h"
#include <cstring>
#include <utility>

using namespace Steinberg;
namespace TrackMesh {
namespace {
constexpr int width = 580, height = 400;
const ViewRect rectangle(0, 0, width, height);
constexpr COLORREF background = RGB(21, 25, 32), foreground = RGB(235, 239, 246), muted = RGB(155, 170, 192);
std::mutex classMutex;
size_t classViews = 0;
void label(HDC dc, HFONT font, const std::wstring& value, RECT area, COLORREF color, UINT flags = DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS) {
    const auto previous = SelectObject(dc, font);
    SetTextColor(dc, color);
    DrawTextW(dc, value.c_str(), static_cast<int>(value.size()), &area, flags);
    SelectObject(dc, previous);
}
}
View::View(std::shared_ptr<Bridge> b) : CPluginView(&rectangle), bridge(std::move(b)) {
    font = CreateFontW(-15, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    titleFont = CreateFontW(-24, 0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    backgroundBrush = CreateSolidBrush(background);
}
View::~View() { removed(); if (font) DeleteObject(font); if (titleFont) DeleteObject(titleFont); if (backgroundBrush) DeleteObject(backgroundBrush); }
tresult PLUGIN_API View::isPlatformTypeSupported(FIDString type) { return type && std::strcmp(type, kPlatformTypeHWND) == 0 ? kResultTrue : kResultFalse; }
tresult PLUGIN_API View::attached(void* parent, FIDString type) {
    if (!parent || window || isPlatformTypeSupported(type) != kResultTrue) return kResultFalse;
    HMODULE module = nullptr;
    GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        reinterpret_cast<LPCWSTR>(&procedure), &module);
    WNDCLASSW klass {};
    klass.lpfnWndProc = procedure;
    klass.hInstance = module;
    klass.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    klass.lpszClassName = L"TrackMeshVST3View-v1";
    {
        std::lock_guard<std::mutex> lock(classMutex);
        if (!RegisterClassW(&klass) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return kResultFalse;
        ++classViews;
        registeredClass = true;
        viewModule = module;
    }
    window = CreateWindowExW(0, klass.lpszClassName, L"TRACKMESH", WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
        0, 0, width, height, static_cast<HWND>(parent), nullptr, module, this);
    if (!window) { removed(); return kResultFalse; }
    const auto make = [&](const wchar_t* cls, const wchar_t* text, DWORD style, int x, int y, int w, int h, int id) {
        const auto control = CreateWindowExW(0, cls, text, WS_CHILD | WS_VISIBLE | style, x, y, w, h, window,
            reinterpret_cast<HMENU>(static_cast<INT_PTR>(id)), module, nullptr);
        SendMessageW(control, WM_SETFONT, reinterpret_cast<WPARAM>(font), TRUE);
        return control;
    };
    uuid = make(L"EDIT", L"", ES_READONLY | ES_AUTOHSCROLL, 24, 105, 532, 24, 10);
    sendButton = make(L"BUTTON", L"Send Snapshot", WS_TABSTOP | BS_OWNERDRAW, 24, 282, 254, 38, 1);
    receiveButton = make(L"BUTTON", L"Receive...", WS_TABSTOP | BS_OWNERDRAW, 302, 282, 254, 38, 2);
    SetTimer(window, 1, 500, nullptr);
    CPluginView::attached(parent, type);
    return kResultOk;
}
tresult PLUGIN_API View::removed() {
    if (window) { KillTimer(window, 1); DestroyWindow(window); window = nullptr; }
    if (registeredClass) {
        std::lock_guard<std::mutex> lock(classMutex);
        if (classViews > 0 && --classViews == 0) UnregisterClassW(L"TrackMeshVST3View-v1", viewModule);
        registeredClass = false;
    }
    sendButton = receiveButton = uuid = nullptr;
    return CPluginView::removed();
}
void View::paint(HWND handle) {
    PAINTSTRUCT paint {};
    const auto dc = BeginPaint(handle, &paint);
    RECT client {};
    GetClientRect(handle, &client);
    const auto brush = CreateSolidBrush(background);
    FillRect(dc, &client, brush); DeleteObject(brush);
    SetBkMode(dc, TRANSPARENT);
    const auto status = bridge->status();
    label(dc, titleFont, L"TRACKMESH", {24, 20, 360, 54}, foreground);
    label(dc, font, status.connection.empty() ? L"CONNECTING" : status.connection, {400, 28, 556, 52}, RGB(111, 218, 193), DT_RIGHT | DT_SINGLELINE);
    label(dc, font, status.name.empty() ? L"Link this project UUID in desktop" : status.name, {24, 63, 556, 87}, foreground);
    label(dc, font, L"PROJECT UUID", {24, 86, 556, 104}, muted);
    label(dc, font, L"LOCAL SNAPSHOT", {24, 151, 556, 170}, muted);
    label(dc, font, status.local.empty() ? L"No local snapshot" : status.local, {24, 173, 556, 193}, foreground);
    label(dc, font, L"REMOTE SNAPSHOT", {24, 207, 556, 226}, muted);
    label(dc, font, status.remote.empty() ? L"No remote announcement" : status.remote, {24, 229, 556, 249}, foreground);
    auto detail = status.state;
    if (!status.author.empty()) detail += L"  /  " + status.author + L"  /  " + status.createdAt;
    label(dc, font, detail, {24, 258, 556, 278}, RGB(111, 218, 193));
    label(dc, font, status.message, {24, 335, 556, 390}, muted, DT_LEFT | DT_WORDBREAK);
    EndPaint(handle, &paint);
}
LRESULT CALLBACK View::procedure(HWND handle, UINT message, WPARAM wparam, LPARAM lparam) {
    auto self = reinterpret_cast<View*>(GetWindowLongPtrW(handle, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
        self = static_cast<View*>(reinterpret_cast<CREATESTRUCTW*>(lparam)->lpCreateParams);
        SetWindowLongPtrW(handle, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
    }
    if (!self) return DefWindowProcW(handle, message, wparam, lparam);
    switch (message) {
        case WM_PAINT: self->paint(handle); return 0;
        case WM_ERASEBKGND: return 1;
        case WM_TIMER: {
            const auto status = self->bridge->status();
            wchar_t text[64] {};
            GetWindowTextW(self->uuid, text, 64);
            if (status.project != text) SetWindowTextW(self->uuid, status.project.c_str());
            EnableWindow(self->sendButton, status.connected && status.linked);
            EnableWindow(self->receiveButton, status.connected && status.linked && status.peerOnline
                && !status.remote.empty() && status.state != L"DIVERGED"
                && status.state != L"RECOVERY_REQUIRED" && status.state != L"RECEIVING"
                && status.state != L"APPLYING");
            InvalidateRect(handle, nullptr, FALSE);
            return 0;
        }
        case WM_COMMAND:
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 1) self->bridge->send();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 2) self->bridge->receive();
            return 0;
        case WM_DRAWITEM: {
            const auto item = reinterpret_cast<DRAWITEMSTRUCT*>(lparam);
            if (item->CtlID != 1 && item->CtlID != 2) break;
            const bool disabled = (item->itemState & ODS_DISABLED) != 0;
            const auto brush = CreateSolidBrush(disabled ? RGB(39, 46, 57) : RGB(51, 94, 89));
            FillRect(item->hDC, &item->rcItem, brush); DeleteObject(brush);
            SetBkMode(item->hDC, TRANSPARENT);
            label(item->hDC, self->font, item->CtlID == 1 ? L"Send Snapshot" : L"Receive...", item->rcItem,
                disabled ? muted : foreground, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
            if (item->itemState & ODS_FOCUS) DrawFocusRect(item->hDC, &item->rcItem);
            return TRUE;
        }
        case WM_CTLCOLOREDIT:
        case WM_CTLCOLORSTATIC: {
            SetTextColor(reinterpret_cast<HDC>(wparam), foreground);
            SetBkColor(reinterpret_cast<HDC>(wparam), background);
            return reinterpret_cast<LRESULT>(self->backgroundBrush);
        }
        case WM_NCDESTROY: {
            self->window = nullptr;
            SetWindowLongPtrW(handle, GWLP_USERDATA, 0);
            return DefWindowProcW(handle, message, wparam, lparam);
        }
    }
    return DefWindowProcW(handle, message, wparam, lparam);
}
}
