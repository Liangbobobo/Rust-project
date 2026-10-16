import json
from mythic_container.MythicCommandBase import *


class LsArguments(TaskArguments):
    def __init__(self, command_line, **kwargs):
        super().__init__(command_line, **kwargs)
        self.args = [
            CommandParameter(
                name="path",
                type=ParameterType.String,
                default_value=".",
                description="Path of directory to list",
                parameter_group_info=[ParameterGroupInfo(required=False, group_name="Default")],
            ),
            CommandParameter(
                name="host",
                type=ParameterType.String,
                default_value="",
                description="Host to list files on",
                parameter_group_info=[ParameterGroupInfo(required=False, group_name="Default")],
            ),
        ]

    async def parse_arguments(self):
        if len(self.command_line) > 0:
            if self.command_line[0] == '{':
                try:
                    temp_json = json.loads(self.command_line)
                    if "host" in temp_json and temp_json["host"]:
                        self.add_arg("host", temp_json["host"])

                    if "full_path" in temp_json and temp_json["full_path"]:
                        self.add_arg("path", temp_json["full_path"])
                    elif "path" in temp_json and "file" in temp_json and temp_json["file"]:
                        parent = temp_json.get("path", "")
                        filename = temp_json.get("file", "")
                        if parent:
                            separator = "\\" if ("\\" in parent or "/" not in parent) else "/"
                            combined = f"{parent.rstrip(separator)}{separator}{filename}"
                            self.add_arg("path", combined)
                        else:
                            self.add_arg("path", filename)
                    elif "path" in temp_json and temp_json["path"]:
                        self.add_arg("path", temp_json["path"])
                    elif "file" in temp_json and temp_json["file"]:
                        self.add_arg("path", temp_json["file"])
                    else:
                        self.add_arg("path", self.command_line.strip())
                except Exception:
                    self.add_arg("path", self.command_line.strip())
            else:
                self.add_arg("path", self.command_line.strip())
        else:
            self.add_arg("path", ".")


class LsCommand(CommandBase):
    cmd = "ls"
    needs_admin = False
    help_cmd = "ls [path]"
    description = "List files and directories"
    version = 1
    author = "@wisconsin"
    argument_class = LsArguments
    supported_ui_features = ["file_browser:list"]
    attributes = CommandAttributes(
        supported_os=[SupportedOS.Windows],
    )

    async def create_go_tasking(self, taskData: PTTaskMessageAllData) -> PTTaskCreateTaskingMessageResponse:
        response = PTTaskCreateTaskingMessageResponse(
            TaskData=taskData,
            Success=True,
        )
        path = taskData.args.get_arg("path")
        host = taskData.args.get_arg("host")
        if host:
            response.DisplayParams = f"Listing: {path} on {host}"
        else:
            response.DisplayParams = f"Listing: {path}"
        return response
