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
        ]

    async def parse_arguments(self):
        if len(self.command_line) > 0:
            self.add_arg("path", self.command_line.strip())
        else:
            self.add_arg("path", ".")


class LsCommand(CommandBase):
    cmd = "ls"
    needs_admin = False
    help_cmd = "ls [path]"
    description = "List files and directories (integrated with Mythic File Browser UI)"
    version = 1
    author = "@wisconsin"
    argument_class = LsArguments
    supported_ui_features = ["file_browser:list", "file_browser:remove"]
    attributes = CommandAttributes(
        supported_os=[SupportedOS.Windows],
    )

    async def create_go_tasking(self, taskData: PTTaskMessageAllData) -> PTTaskCreateTaskingMessageResponse:
        response = PTTaskCreateTaskingMessageResponse(
            TaskData=taskData,
            Success=True,
        )
        path = taskData.args.get_arg("path")
        response.DisplayParams = f"Listing: {path}"
        return response
