from mythic_container.MythicCommandBase import *
from mythic_container.MythicRPC import *


class DownloadArguments(TaskArguments):
    def __init__(self, command_line, **kwargs):
        super().__init__(command_line, **kwargs)
        self.args = [
            CommandParameter(
                name="file_path",
                type=ParameterType.String,
                description="Target path of file on remote machine to download",
                parameter_group_info=[ParameterGroupInfo(required=True, group_name="Default")],
            ),
        ]

    async def parse_arguments(self):
        if len(self.command_line) == 0:
            raise ValueError("Must provide remote file path to download")
        self.add_arg("file_path", self.command_line.strip())


class DownloadCommand(CommandBase):
    cmd = "download"
    needs_admin = False
    help_cmd = "download [file_path]"
    description = "Download file from remote host using Mythic 4.0 pure byte-offset streaming"
    version = 1
    author = "@wisconsin"
    argument_class = DownloadArguments
    supported_ui_features = ["file_browser:download"]
    attributes = CommandAttributes(
        supported_os=[SupportedOS.Windows],
    )

    async def create_go_tasking(self, taskData: PTTaskMessageAllData) -> PTTaskCreateTaskingMessageResponse:
        response = PTTaskCreateTaskingMessageResponse(
            TaskData=taskData,
            Success=True,
        )
        file_path = taskData.args.get_arg("file_path")
        response.DisplayParams = f"Downloading: {file_path}"
        return response
