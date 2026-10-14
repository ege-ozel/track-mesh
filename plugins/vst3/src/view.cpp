#include "view.h"
#include "plugin.h"
#include <algorithm>
#include <cstring>
#include <utility>

using namespace Steinberg;
namespace TrackMesh {
namespace {
constexpr int width = 580, height = 440;
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
View::View(std::shared_ptr<Bridge> b, Plugin* p) : CPluginView(&rectangle), bridge(std::move(b)), plugin(p) {
    plugin->addRef();
    font = CreateFontW(-15, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    titleFont = CreateFontW(-24, 0, 0, 0, FW_SEMIBOLD, FALSE, FALSE, FALSE, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    smallFont = CreateFontW(-13, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE, DEFAULT_CHARSET, 0, 0, CLEARTYPE_QUALITY, 0, L"Segoe UI");
    backgroundBrush = CreateSolidBrush(background);
}
View::~View() { removed(); if (font) DeleteObject(font); if (titleFont) DeleteObject(titleFont); if (smallFont) DeleteObject(smallFont); if (backgroundBrush) DeleteObject(backgroundBrush); plugin->release(); }
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
    picker = make(L"COMBOBOX", L"", WS_TABSTOP | CBS_DROPDOWNLIST | WS_VSCROLL, 24, 105, 410, 210, 10);
    chooseButton = make(L"BUTTON", L"Use project", WS_TABSTOP, 450, 105, 106, 28, 11);
    changeButton = make(L"BUTTON", L"Change project", WS_TABSTOP, 420, 63, 136, 28, 12);
    openButton = make(L"BUTTON", L"Open TrackMesh", WS_TABSTOP, 24, 170, 200, 32, 13);
    sendButton = make(L"BUTTON", L"Share current version", WS_TABSTOP | BS_OWNERDRAW, 24, 282, 254, 38, 1);
    receiveButton = make(L"BUTTON", L"Get collaborator's version", WS_TABSTOP | BS_OWNERDRAW, 302, 282, 254, 38, 2);
    autoShareCheckbox = make(L"BUTTON", L"Share automatically when the DAW project is saved", WS_TABSTOP | BS_AUTOCHECKBOX, 24, 328, 532, 26, 14);
    refresh();
    SetTimer(window, 1, 250, nullptr);
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
    sendButton = receiveButton = picker = chooseButton = changeButton = openButton = nullptr;
    autoShareCheckbox = nullptr;
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
    const bool selecting = !status.linked || changing;
    label(dc, font, selecting ? L"Choose a linked TrackMesh project" : status.name, {24, 63, 410, 90}, foreground);
    if (!selecting) {
        label(dc, font, status.line, {24, 105, 556, 163}, foreground, DT_LEFT | DT_WORDBREAK);
        label(dc, font, status.createdAt, {24, 173, 556, 195}, muted);
        if (!status.author.empty()) label(dc, font, L"Collaborator: " + status.author, {24, 204, 556, 224}, muted);
        label(dc, smallFont, L"Project: " + status.project, {24, 229, 556, 249}, muted);
        if (!status.local.empty() || !status.remote.empty())
            label(dc, smallFont, L"Version: " + (status.local.empty() ? L"none" : status.local.substr(0, 8)) + L" / "
                + (status.remote.empty() ? L"none" : status.remote.substr(0, 8)), {24, 253, 556, 274}, muted);
    } else {
        const auto text = !status.pickerSupported ? (status.connected ? status.line : status.message)
            : choices.empty() ? L"Link this project's folder in TrackMesh first" : L"Choose the project whose folder contains this DAW project.";
        label(dc, font, text, {24, 139, 556, 166}, muted);
        if (status.projectsMore) label(dc, font, L"Showing the first 64 projects linked in TrackMesh.", {24, 214, 556, 238}, muted);
    }
    label(dc, font, selectionError.empty() ? status.message : selectionError, {24, 366, 556, 430}, muted, DT_LEFT | DT_WORDBREAK);
    EndPaint(handle, &paint);
}
void View::refresh() {
    const auto status = bridge->status();
    bool same = choices.size() == status.projects.size();
    for (size_t i = 0; same && i < choices.size(); ++i)
        same = choices[i].id == status.projects[i].id && choices[i].name == status.projects[i].name && choices[i].folder == status.projects[i].folder;
    if (!same) {
        const auto index = SendMessageW(picker, CB_GETCURSEL, 0, 0);
        const auto selected = index >= 0 && static_cast<size_t>(index) < choices.size() ? choices[static_cast<size_t>(index)].id : ProjectId{};
        choices = status.projects;
        SendMessageW(picker, CB_RESETCONTENT, 0, 0);
        for (size_t i = 0; i < choices.size(); ++i) {
            auto text = choices[i].name;
            if (std::count_if(choices.begin(), choices.end(), [&](const auto& choice) { return choice.name == text; }) > 1)
                text += L" (" + choices[i].folder + L")";
            SendMessageW(picker, CB_ADDSTRING, 0, reinterpret_cast<LPARAM>(text.c_str()));
            if (choices[i].id == selected) SendMessageW(picker, CB_SETCURSEL, i, 0);
        }
    }
    const bool selecting = !status.linked || changing;
    ShowWindow(picker, selecting && !choices.empty() ? SW_SHOW : SW_HIDE);
    ShowWindow(chooseButton, selecting && !choices.empty() ? SW_SHOW : SW_HIDE);
    ShowWindow(changeButton, status.linked ? SW_SHOW : SW_HIDE);
    SetWindowTextW(changeButton, changing ? L"Cancel" : L"Change project");
    ShowWindow(openButton, selecting ? SW_SHOW : SW_HIDE);
    ShowWindow(sendButton, selecting ? SW_HIDE : SW_SHOW);
    ShowWindow(receiveButton, selecting ? SW_HIDE : SW_SHOW);
    ShowWindow(autoShareCheckbox, !selecting && status.autoShareSupported ? SW_SHOW : SW_HIDE);
    SendMessageW(autoShareCheckbox, BM_SETCHECK, status.autoShare ? BST_CHECKED : BST_UNCHECKED, 0);
    EnableWindow(autoShareCheckbox, status.connected && status.autoShareSupported);
    EnableWindow(sendButton, status.connected && status.shareEnabled);
    EnableWindow(receiveButton, status.connected && status.getEnabled);
    EnableWindow(openButton, status.connected && status.desktopAvailable);
    EnableWindow(chooseButton, status.connected && SendMessageW(picker, CB_GETCURSEL, 0, 0) != CB_ERR);
    InvalidateRect(window, nullptr, FALSE);
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
            self->refresh();
            return 0;
        }
        case WM_COMMAND:
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 1) self->bridge->send();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 2) self->bridge->receive();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 13) self->bridge->receive();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 14)
                self->bridge->setAutoShare(SendMessageW(self->autoShareCheckbox, BM_GETCHECK, 0, 0) == BST_CHECKED);
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 12) { self->changing = !self->changing; self->selectionError.clear(); self->refresh(); }
            if (HIWORD(wparam) == CBN_SELCHANGE && LOWORD(wparam) == 10) self->refresh();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 11) {
                const auto index = SendMessageW(self->picker, CB_GETCURSEL, 0, 0);
                if (index >= 0 && static_cast<size_t>(index) < self->choices.size()) {
                    if (self->plugin->selectProject(self->choices[static_cast<size_t>(index)].id)) {
                        self->changing = false; self->selectionError.clear();
                    } else self->selectionError = L"The DAW could not mark this project choice for saving. Project choice was not changed.";
                    self->refresh();
                }
            }
            return 0;
        case WM_DRAWITEM: {
            const auto item = reinterpret_cast<DRAWITEMSTRUCT*>(lparam);
            if (item->CtlID != 1 && item->CtlID != 2) break;
            const bool disabled = (item->itemState & ODS_DISABLED) != 0;
            const auto brush = CreateSolidBrush(disabled ? RGB(39, 46, 57) : RGB(51, 94, 89));
            FillRect(item->hDC, &item->rcItem, brush); DeleteObject(brush);
            SetBkMode(item->hDC, TRANSPARENT);
            label(item->hDC, self->font, item->CtlID == 1 ? L"Share current version" : L"Get collaborator's version", item->rcItem,
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
