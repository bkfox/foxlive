import("stdfaust.lib");

maxDuration = nentry("max duration", 10, 0, 20, 0.1);
duration = nentry("duration", 5, 0, 10, 0.1);
feedback = nentry("feedback", 0.5, 0, 1, 0.01);
process = ef.echo(maxDuration,duration,feedback);

