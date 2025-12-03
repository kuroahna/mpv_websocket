-- mpv_websocket
-- https://github.com/kuroahna/mpv_websocket

local use_secondary_subs = false

local utils = require("mp.utils")

local platform = mp.get_property_native("platform")

local config_file_path = mp.find_config_file("mpv.conf")
local config_folder_path, config_file = utils.split_path(config_file_path)
local mpv_websocket_path =
  utils.join_path(config_folder_path, platform == "windows" and "mpv_websocket.exe" or "mpv_websocket")
local initialised_websocket

local _, err = utils.file_info(config_file_path)
if err then
  error("failed to open mpv config file `" .. config_file_path .. "`")
end

local _, err = utils.file_info(mpv_websocket_path)
if err then
  error("failed to open mpv_websocket")
end

local function find_mpv_socket(config_file_path)
  local file = io.open(config_file_path, "r")
  if file == nil then
    error("failed to read mpv config file `" .. config_file_path .. "`")
  end

  local mpv_socket
  for line in file:lines() do
    mpv_socket = line:match("^input%-ipc%-server%s*=%s*(%g+)%s*")
    if mpv_socket then
      break
    end
  end

  file:close()

  if not mpv_socket then
    error("input-ipc-server option does not exist in `" .. config_file_path .. "`")
  end

  return mpv_socket
end

local mpv_socket = find_mpv_socket(config_file_path)
if platform == "windows" then
  mpv_socket = "\\\\.\\pipe" .. mpv_socket:gsub("/", "\\")
end

local function start_websocket()
  local args = {
    mpv_websocket_path,
    "-m",
    mpv_socket,
    "-w",
    "6677",
  }

  if use_secondary_subs then
    table.insert(args, "-s")
  end

  initialised_websocket = mp.command_native_async({
    name = "subprocess",
    playback_only = false,
    capture_stdout = true,
    capture_stderr = true,
    args = args,
  })
end

local function end_websocket()
  mp.abort_async_command(initialised_websocket)
  initialised_websocket = nil
end

local function toggle_websocket()
  local paused = mp.get_property_bool("pause")
  if initialised_websocket and paused then
    end_websocket()
  elseif not initialised_websocket and not paused then
    start_websocket()
  end
end

local function toggle_subs_type()
  if use_secondary_subs then
    use_secondary_subs = false
  else
    use_secondary_subs = true
  end
  if initialised_websocket then
    end_websocket()
    start_websocket()
  end
end

mp.register_script_message("togglewebsocket", toggle_websocket)
mp.register_script_message("togglesubstype", toggle_subs_type)
start_websocket()
