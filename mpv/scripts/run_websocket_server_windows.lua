-- mpv_websocket
-- https://github.com/kuroahna/mpv_websocket

local msg = require("mp.msg")
local utils = require("mp.utils")

local config_file_path = mp.find_config_file("mpv.conf")
local config_folder_path, config_file = utils.split_path(config_file_path)
local mpv_websocket_path = utils.join_path(config_folder_path, "mpv_websocket.exe")
local initialised_websocket

local function find_mpv_socket(conffile)
  local f = conffile and io.open(conffile, "r")
  if f == nil then
    -- config not found
    msg.debug(conffile .. " not found.")
  else
    for line in f:lines() do
      if line:sub(#line) == "\r" then
        line = line:sub(1, #line - 1)
      end
      if string.find(line, "#") ~= 1 then
        local eqpos = string.find(line, "=")
        if eqpos ~= nil then
          local key = string.sub(line, 1, eqpos - 1)
          local val = string.sub(line, eqpos + 1)

          if key == "input-ipc-server" then
            local percentpos = string.find(val, "%%", 2)
            if percentpos ~= nil then
              val = string.sub(val, percentpos + 1)
            end
            msg.debug("found mpv input socket at " .. val)

            return val
          end
        end
      end
    end
    io.close(f)
  end
  -- fallback to old, hardcoded location
  return "/tmp/mpv-socket"
end

local function start_websocket()
  initialised_websocket = mp.command_native_async({
    name = "subprocess",
    playback_only = false,
    capture_stdout = true,
    capture_stderr = true,
    args = {
      mpv_websocket_path,
      "-m",
      "\\\\.\\pipe" .. find_mpv_socket(config_file_path):gsub("/", "\\"),
      "-w",
      "6677",
    },
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

mp.register_script_message("togglewebsocket", toggle_websocket)
start_websocket()
