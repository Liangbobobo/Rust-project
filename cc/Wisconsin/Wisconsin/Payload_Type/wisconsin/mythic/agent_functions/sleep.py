from mythic_container.MythicCommandBase import *


class SleepArguments(TaskArguments):
    def __init__(self, command_line, **kwargs):
        super().__init__(command_line, **kwargs)
        self.args = [
            CommandParameter(
                name="interval",
                type=ParameterType.Number,
                description="Sleep interval in seconds",
                parameter_group_info=[ParameterGroupInfo(required=True, group_name="Default")],
            ),
            CommandParameter(
                name="jitter",
                type=ParameterType.Number,
                default_value=15,
                description="Sleep jitter percentage (0-100)",
                parameter_group_info=[ParameterGroupInfo(required=False, group_name="Default")],
            ),
        ]

    async def parse_arguments(self):
        if len(self.command_line) == 0:
            raise ValueError("Must provide at least sleep interval in seconds")
        parts = self.command_line.split()
        self.add_arg("interval", int(parts[0]))
        if len(parts) > 1:
            self.add_arg("jitter", int(parts[1]))
        else:
            self.add_arg("jitter", 15)


class SleepCommand(CommandBase):
    cmd = "sleep"
    needs_admin = False
    help_cmd = "sleep [seconds] [jitter_percent]"
    description = "Update the sleep interval and jitter percentage of the agent"
    version = 1
    author = "@wisconsin"
    argument_class = SleepArguments
    attributes = CommandAttributes(
        supported_os=[SupportedOS.Windows],
    )

    async def create_go_tasking(self, taskData: PTTaskMessageAllData) -> PTTaskCreateTaskingMessageResponse:
        response = PTTaskCreateTaskingMessageResponse(
            TaskData=taskData,
            Success=True,
        )
        interval = taskData.args.get_arg("interval")
        jitter = taskData.args.get_arg("jitter")
        taskData.args.command_line = f"{interval} {jitter}"
        response.DisplayParams = f"interval: {interval}s, jitter: {jitter}%"
        return response
