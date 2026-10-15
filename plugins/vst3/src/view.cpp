#include "view.h"
#include "plugin.h"
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <utility>
#include <uxtheme.h>

using namespace Steinberg;
namespace TrackMesh {
namespace {
constexpr int width = 580, height = 440;
const ViewRect rectangle(0, 0, width, height);
// One palette for the whole view, matching the TrackMesh window.
constexpr COLORREF background = RGB(21, 25, 32), foreground = RGB(235, 239, 246), muted = RGB(155, 170, 192);
constexpr COLORREF primaryFill = RGB(51, 94, 89), secondaryFill = RGB(47, 58, 73), disabledFill = RGB(39, 46, 57), accent = RGB(111, 218, 193),
    warn = RGB(244, 195, 96), danger = RGB(255, 112, 112);
std::mutex classMutex;
size_t classViews = 0;
void label(HDC dc, HFONT font, const std::wstring& value, RECT area, COLORREF color, UINT flags = DT_LEFT | DT_SINGLELINE | DT_END_ELLIPSIS) {
    const auto previous = SelectObject(dc, font);
    SetTextColor(dc, color);
    DrawTextW(dc, value.c_str(), static_cast<int>(value.size()), &area, flags);
    SelectObject(dc, previous);
}
// The plugin follows the TrackMesh window's language; older agents send none, which reads as English.
std::wstring pick(const ViewStatus& status, const wchar_t* english, const wchar_t* turkish) {
    return status.language == L"tr" ? turkish : english;
}
void setText(HWND control, const std::wstring& text) {
    wchar_t current[128] {};
    GetWindowTextW(control, current, 128);
    if (text != current) { SetWindowTextW(control, text.c_str()); InvalidateRect(control, nullptr, FALSE); }
}
// RFC 3339 UTC from the agent ("2026-10-14T12:34:56+00:00") as local "14.10.2026 15:34", as in the app; empty if unreadable.
std::wstring localTime(const std::wstring& value) {
    tm utc {};
    if (swscanf_s(value.c_str(), L"%4d-%2d-%2dT%2d:%2d", &utc.tm_year, &utc.tm_mon, &utc.tm_mday, &utc.tm_hour, &utc.tm_min) != 5) return {};
    utc.tm_year -= 1900;
    utc.tm_mon -= 1;
    const auto seconds = _mkgmtime64(&utc);
    tm local {};
    if (seconds == -1 || _localtime64_s(&local, &seconds) != 0) return {};
    wchar_t text[32] {};
    return wcsftime(text, 32, L"%d.%m.%Y %H:%M", &local) ? std::wstring(text) : std::wstring();
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
    SetWindowTheme(picker, L"DarkMode_CFD", nullptr); // dark closed field on Windows 10 1809 and later
    chooseButton = make(L"BUTTON", L"Use project", WS_TABSTOP | BS_OWNERDRAW, 450, 105, 106, 28, 11);
    changeButton = make(L"BUTTON", L"Change project", WS_TABSTOP | BS_OWNERDRAW, 420, 63, 136, 28, 12);
    openButton = make(L"BUTTON", L"Open TrackMesh", WS_TABSTOP | BS_OWNERDRAW, 24, 170, 200, 32, 13);
    sendButton = make(L"BUTTON", L"Share current version", WS_TABSTOP | BS_OWNERDRAW, 24, 282, 254, 38, 1);
    receiveButton = make(L"BUTTON", L"Get collaborator's version", WS_TABSTOP | BS_OWNERDRAW, 302, 282, 254, 38, 2);
    autoShareCheckbox = make(L"BUTTON", L"Share automatically when the DAW project is saved", WS_TABSTOP | BS_OWNERDRAW, 24, 328, 532, 26, 14);
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
    label(dc, titleFont, L"TRACKMESH", {24, 20, 300, 54}, foreground);
    const bool selecting = !status.linked || changing;
    // Same words as the Desktop card badge, never the raw route.
    const auto connection = !status.connected ? pick(status, L"Connecting…", L"Bağlanıyor…")
        : selecting ? std::wstring()
        : status.peerOnline ? pick(status, L"Connected", L"Bağlı") + (status.author.empty() ? L"" : L" · " + status.author)
        : status.author.empty() ? pick(status, L"No collaborator yet", L"Henüz iş arkadaşı yok") : pick(status, L"Not connected", L"Bağlı değil");
    label(dc, font, connection, {300, 28, 556, 52}, status.peerOnline ? accent : muted, DT_RIGHT | DT_SINGLELINE | DT_END_ELLIPSIS);
    label(dc, font, selecting ? pick(status, L"Which TrackMesh project is this?", L"Bu hangi TrackMesh projesi?") : status.name, {24, 63, 410, 90}, foreground);
    if (!selecting) {
        label(dc, font, status.presenceWarning.empty() ? status.line : status.presenceWarning, {24, 105, 556, 163},
            status.presenceWarning.empty() ? foreground : status.presenceDanger ? danger : warn, DT_LEFT | DT_WORDBREAK);
        // The collaborator's latest version, in local time.
        const auto when = localTime(status.createdAt);
        if (!status.author.empty() && !when.empty())
            label(dc, font, pick(status, L"Latest version from ", L"Son sürüm: ") + status.author + L" · " + when, {24, 173, 556, 195}, muted);
    } else {
        const auto text = !status.pickerSupported ? (status.connected ? status.line : status.message)
            : choices.empty() ? pick(status, L"Add this project's folder in TrackMesh first (Add project).", L"Önce bu projenin klasörünü TrackMesh'e ekle (Proje ekle).")
            : pick(status, L"Choose the TrackMesh project whose folder holds this DAW project.", L"Bu DAW projesinin klasörünü içeren TrackMesh projesini seç.");
        label(dc, font, text, {24, 139, 556, 166}, muted);
        if (status.projectsMore) label(dc, font, pick(status, L"Showing the first 64 TrackMesh projects.", L"TrackMesh'teki ilk 64 proje gösteriliyor."), {24, 214, 556, 238}, muted);
    }
    // Red for a refusal, amber when the agent is away, gray otherwise; never the status line twice.
    const auto message = selectionError.empty() ? status.message : selectionError;
    const bool repeated = !selecting && (message == status.line || message == status.presenceWarning);
    const auto tone = !selectionError.empty() || status.state == L"ACTION REFUSED" ? danger : status.state == L"AGENT UNAVAILABLE" ? warn : muted;
    if (!repeated) label(dc, font, message, {24, 366, 556, 430}, tone, DT_LEFT | DT_WORDBREAK);
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
    setText(changeButton, changing ? pick(status, L"Cancel", L"Vazgeç") : pick(status, L"Change project", L"Projeyi değiştir"));
    setText(chooseButton, pick(status, L"Use project", L"Bu projeyi kullan"));
    setText(openButton, pick(status, L"Open TrackMesh", L"TrackMesh'i aç"));
    setText(autoShareCheckbox, pick(status, L"Share when I save in the DAW", L"DAW'da kaydedince paylaş"));
    ShowWindow(openButton, selecting ? SW_SHOW : SW_HIDE);
    ShowWindow(sendButton, selecting ? SW_HIDE : SW_SHOW);
    ShowWindow(receiveButton, selecting ? SW_HIDE : SW_SHOW);
    ShowWindow(autoShareCheckbox, !selecting && status.autoShareSupported ? SW_SHOW : SW_HIDE);
    EnableWindow(autoShareCheckbox, status.connected && status.autoShareSupported);
    EnableWindow(sendButton, status.connected && status.shareEnabled);
    EnableWindow(receiveButton, status.connected && status.getEnabled);
    EnableWindow(openButton, status.connected && status.desktopAvailable);
    EnableWindow(chooseButton, status.connected && SendMessageW(picker, CB_GETCURSEL, 0, 0) != CB_ERR);
    // Owner-drawn buttons repaint only when asked; ask when their words change.
    const auto labels = status.language + L"|" + status.getLabel + L"|" + status.state + (status.autoShare ? L"|on" : L"|off");
    if (labels != drawnLabels) {
        drawnLabels = labels;
        InvalidateRect(sendButton, nullptr, FALSE);
        InvalidateRect(receiveButton, nullptr, FALSE);
        InvalidateRect(autoShareCheckbox, nullptr, FALSE);
    }
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
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 1) {
                const auto status = self->bridge->status();
                if (status.shareConfirmation.empty() || MessageBoxW(handle, status.shareConfirmation.c_str(),
                        pick(status, L"Share current version?", L"Güncel sürüm paylaşılsın mı?").c_str(), MB_OKCANCEL | MB_ICONWARNING) == IDOK)
                    self->bridge->send();
            }
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 2) self->bridge->receive();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 13) self->bridge->receive();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 14)
                self->bridge->setAutoShare(!self->bridge->status().autoShare);
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 12) { self->changing = !self->changing; self->selectionError.clear(); self->refresh(); }
            if (HIWORD(wparam) == CBN_SELCHANGE && LOWORD(wparam) == 10) self->refresh();
            if (HIWORD(wparam) == BN_CLICKED && LOWORD(wparam) == 11) {
                const auto index = SendMessageW(self->picker, CB_GETCURSEL, 0, 0);
                if (index >= 0 && static_cast<size_t>(index) < self->choices.size()) {
                    if (self->plugin->selectProject(self->choices[static_cast<size_t>(index)].id)) {
                        self->changing = false; self->selectionError.clear();
                    } else self->selectionError = pick(self->bridge->status(), L"The DAW could not save this project choice. Nothing was changed.", L"DAW bu proje seçimini kaydedemedi. Hiçbir şey değişmedi.");
                    self->refresh();
                }
            }
            return 0;
        case WM_DRAWITEM: {
            const auto item = reinterpret_cast<DRAWITEMSTRUCT*>(lparam);
            const bool disabled = (item->itemState & ODS_DISABLED) != 0;
            if (item->CtlID == 14) {
                // Share on save: a dark box, filled with the accent and a tick when on.
                const auto status = self->bridge->status();
                const auto fill = CreateSolidBrush(background);
                FillRect(item->hDC, &item->rcItem, fill); DeleteObject(fill);
                RECT box {item->rcItem.left, item->rcItem.top + 4, item->rcItem.left + 18, item->rcItem.top + 22};
                const auto boxBrush = CreateSolidBrush(status.autoShare && !disabled ? accent : disabledFill);
                FillRect(item->hDC, &box, boxBrush); DeleteObject(boxBrush);
                const auto edge = CreateSolidBrush(disabled ? disabledFill : muted);
                FrameRect(item->hDC, &box, edge); DeleteObject(edge);
                SetBkMode(item->hDC, TRANSPARENT);
                if (status.autoShare) label(item->hDC, self->font, L"✓", box, background, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                wchar_t current[128] {};
                GetWindowTextW(item->hwndItem, current, 128);
                label(item->hDC, self->font, current, {box.right + 10, item->rcItem.top, item->rcItem.right, item->rcItem.bottom}, disabled ? muted : foreground, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
                if ((item->itemState & ODS_FOCUS) && !(item->itemState & ODS_NOFOCUSRECT)) DrawFocusRect(item->hDC, &item->rcItem);
                return TRUE;
            }
            if (item->CtlID < 1 || (item->CtlID > 2 && (item->CtlID < 11 || item->CtlID > 13))) break;
            // One primary button: Get when the collaborator has a newer version, otherwise Share.
            const auto status = self->bridge->status();
            const bool getFirst = status.state == L"REMOTE_AHEAD";
            const bool secondary = item->CtlID == 12 || (item->CtlID == 1 && getFirst) || (item->CtlID == 2 && !getFirst);
            const auto brush = CreateSolidBrush(disabled ? disabledFill : secondary ? secondaryFill : primaryFill);
            FillRect(item->hDC, &item->rcItem, brush); DeleteObject(brush);
            SetBkMode(item->hDC, TRANSPARENT);
            std::wstring text;
            if (item->CtlID == 1) text = pick(status, L"Share current version", L"Güncel sürümü paylaş");
            // Get opens TrackMesh on this project, hence the ellipsis.
            else if (item->CtlID == 2) text = (status.getLabel.empty() ? pick(status, L"Get collaborator's version", L"İş arkadaşının sürümünü al") : status.getLabel) + L"…";
            else {
                wchar_t current[128] {};
                GetWindowTextW(item->hwndItem, current, 128);
                text = current;
            }
            label(item->hDC, self->font, text, item->rcItem, disabled ? muted : foreground, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS);
            // Keyboard focus only; a mouse press draws no focus box.
            if ((item->itemState & ODS_FOCUS) && !(item->itemState & ODS_NOFOCUSRECT)) DrawFocusRect(item->hDC, &item->rcItem);
            return TRUE;
        }
        case WM_CTLCOLOREDIT:
        case WM_CTLCOLORLISTBOX:
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
