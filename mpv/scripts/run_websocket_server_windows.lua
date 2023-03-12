local utils = require 'mp.utils'

local config_file_path = mp.find_config_file("mpv.conf")
local config_folder_path, config_file = utils.split_path(config_file_path)
local mpv_websocket_path = utils.join_path(config_folder_path, "mpv_websocket.exe")

mp.command_native_async({
    name = "subprocess",
    playback_only = false,
    capture_stdout = true,
    capture_stderr = true,
    args = {
        mpv_websocket_path,
        "-m",
        "\\\\.\\pipe\\tmp\\mpv-socket",
        "-w",
        "6677",
    },
})
