from mythic_container.MythicCommandBase import *
from mythic_container.MythicRPC import *


class UploadArguments(TaskArguments):
    def __init__(self, command_line, **kwargs):
        super().__init__(command_line, **kwargs)
        self.args = [
            CommandParameter(
                name="file",
                type=ParameterType.File,
                description="File from operator to upload to target",
                parameter_group_info=[ParameterGroupInfo(required=True, group_name="Default")],
            ),
            CommandParameter(
                name="remote_path",
                type=ParameterType.String,
                description="Destination full path on target host",
                parameter_group_info=[ParameterGroupInfo(required=True, group_name="Default")],
            ),
        ]

    async def parse_arguments(self):
        if len(self.command_line) > 0 and self.command_line[0] == "{":
            self.load_args_from_json_string(self.command_line)
        else:
            raise ValueError("Upload requires JSON or UI modal arguments containing file and remote_path")


class UploadCommand(CommandBase):
    cmd = "upload"
    needs_admin = False
    help_cmd = "upload"
    description = "Upload file from Mythic server to target host using Mythic 4.0 chunked upload"
    version = 1
    author = "@wisconsin"
    argument_class = UploadArguments
    supported_ui_features = ["file_browser:upload"]
    attributes = CommandAttributes(
        supported_os=[SupportedOS.Windows],
    )

    async def create_go_tasking(self, taskData: PTTaskMessageAllData) -> PTTaskCreateTaskingMessageResponse:
        response = PTTaskCreateTaskingMessageResponse(
            TaskData=taskData,
            Success=True,
        )
        file_id = taskData.args.get_arg("file")
        remote_path = taskData.args.get_arg("remote_path")
        response.DisplayParams = f"Uploading file ({file_id}) to: {remote_path}"
        return response
