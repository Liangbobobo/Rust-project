from mythic_container.MythicCommandBase import *


class CdArguments(TaskArguments):
    def __init__(self, command_line, **kwargs):
        super().__init__(command_line, **kwargs)
        self.args = [
            CommandParameter(
                name="path",
                type=ParameterType.String,
                description="Target directory path to switch to",
                parameter_group_info=[ParameterGroupInfo(required=True, group_name="Default")],
            ),
        ]

    async def parse_arguments(self):
        if len(self.command_line) == 0:
            raise ValueError("Must provide a path to cd into")
        self.add_arg("path", self.command_line.strip())


class CdCommand(CommandBase):
    cmd = "cd"
    needs_admin = False
    help_cmd = "cd [path]"
    description = "Change current working directory"
    version = 1
    author = "@wisconsin"
    argument_class = CdArguments
    attributes = CommandAttributes(
        supported_os=[SupportedOS.Windows],
    )

    async def create_go_tasking(self, taskData: PTTaskMessageAllData) -> PTTaskCreateTaskingMessageResponse:
        response = PTTaskCreateTaskingMessageResponse(
            TaskData=taskData,
            Success=True,
        )
        path = taskData.args.get_arg("path")
        response.DisplayParams = f"Changing directory to: {path}"
        return response
