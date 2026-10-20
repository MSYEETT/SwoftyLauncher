// struct: WeatherTypeRange
// subsystem: session
// size: 0x14 L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8d3 (type*5*4 indexing)
// notes: per weather type record at TimeOfDaySystem+0xb378. setupSession copies the track config's
// per-type weight into +0x00; the builder draws a state's intensity uniformly in [intensity_min,
// intensity_max]. Raised by ce-weather.
struct WeatherTypeRange {
    /* 0x000 */ float weight;           // L2 decomp:re/decomp/1438b7350.c;xref:0x1438b74c1 copied from the track weather config (+0x40.. per type) by setupSession
    /* 0x00c */ float intensity_min;    // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8d7 lower bound of the intensity draw
    /* 0x010 */ float intensity_max;    // L2 decomp:re/decomp/14461bea0.c;xref:0x14461c8dd upper bound of the intensity draw
};
