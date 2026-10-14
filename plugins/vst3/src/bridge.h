#pragma once
#include <array>
#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <vector>
#include <windows.h>

namespace TrackMesh {
using ProjectId = std::array<unsigned char, 16>;
struct ProjectChoice {
    ProjectId id {};
    std::wstring name, folder;
};
struct ViewStatus {
    std::wstring project, name, local, remote, author, createdAt, connection, state, message;
    bool connected {false};
    bool linked {false};
    bool peerOnline {false};
    std::wstring line;
    std::vector<ProjectChoice> projects;
    bool pickerSupported {false}, projectsMore {false}, desktopAvailable {false};
    bool shareEnabled {false}, getEnabled {false};
};
struct BridgeState {
    std::mutex mutex;
    ProjectId project {};
    ViewStatus view;
    std::atomic<uint64_t> generation {0}, save {0}, send {0}, receive {0};
    std::atomic<bool> ending {false};
    HANDLE wake {nullptr};
    BridgeState();
    ~BridgeState();
};
class Bridge {
public:
    Bridge();
    ~Bridge();
    bool start();
    void stop() noexcept;
    void setProject(const ProjectId& project);
    ProjectId project() const;
    void saveObserved() noexcept;
    void send() noexcept;
    void receive() noexcept;
    ViewStatus status() const;
private:
    std::shared_ptr<BridgeState> state;
    bool started {false};
};
ProjectId newProjectId();
bool validProjectId(const ProjectId& project);
ProjectId parseProjectId(const std::string& text);
std::string projectText(const ProjectId& project);
std::wstring wide(const std::string& text);
}
